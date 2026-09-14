use actix_cors::Cors;
use actix_web::{post, web, App, HttpResponse, HttpServer, Responder, Result as ActixResult};
use anyhow::{anyhow, Result};
use bitcoin::absolute::LockTime;
use bitcoin::consensus::encode::deserialize;
use bitcoin::consensus::{Decodable, Encodable};
use bitcoin::hashes::Hash;
use bitcoin::io::Write;
use bitcoin::p2p::message_blockdata::Inventory;
use bitcoin::BlockHash;
use bitcoin::{Amount, Transaction, Txid};
use clap::Parser;
use env_logger;
use itertools::Itertools;
use log::{debug, info};
use rand;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tokio::time::sleep;

trait FromHex: Sized {
    fn from_hex(hex_str: &str) -> Result<Self>;
}

impl FromHex for Txid {
    fn from_hex(hex_str: &str) -> Result<Self> {
        Ok(Txid::from_byte_array(
            <&[u8] as TryInto<[u8; 32]>>::try_into(&hex::decode(hex_str)?)?,
        ))
    }
}

const UPDATE_INTERVAL: Duration = Duration::from_secs(2);
const TEMPLATE_CACHE_DURATION: Duration = Duration::from_secs(30);
const BLOCK_CONFIRM_TARGETS: [u32; 3] = [1, 2, 3]; // Number of blocks to target
const MAX_BLOCK_WEIGHT: u32 = 4_000_000;
// Block header constants
const VERSION: i32 = 1;
const BLOCK_HEADER_SIZE: usize = 80;
const MIN_FEE_RATE: f64 = 1.0; // sat/vB
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    daemon_rpc_url: String,
    #[arg(long)]
    auth: Option<String>,
    #[arg(long)]
    p2p_addr: Option<String>,
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    host: String,
    #[arg(long, env = "PORT", default_value_t = 8081)]
    port: u16,
}

#[derive(Debug)]
struct P2PConnection {
    stream: tokio::net::TcpStream,
    version_received: bool,
    verack_received: bool,
}

const PROTOCOL_VERSION: i32 = 70016;
const DEFAULT_SERVICES: u64 = 0;
const USER_AGENT: &str = "/memshrew:0.1.0/";

impl P2PConnection {
    async fn new(addr: &str) -> Result<Self> {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        Ok(Self {
            stream,
            version_received: false,
            verack_received: false,
        })
    }

    async fn send_version(&mut self) -> Result<()> {
        let version_msg = VersionMessage {
            version: PROTOCOL_VERSION,
            services: DEFAULT_SERVICES,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            addr_recv: NetAddress::default(),
            addr_from: NetAddress::default(),
            nonce: rand::random(),
            user_agent: USER_AGENT.to_string(),
            start_height: 0,
            relay: true,
        };
        self.send_message("version", &version_msg).await
    }

    async fn send_message<T: Encodable>(&mut self, command: &str, payload: &T) -> Result<()> {
        let mut msg = Vec::new();
        // Magic value for mainnet
        msg.extend_from_slice(&[0xF9, 0xBE, 0xB4, 0xD9]);
        // Command name (padded with 0s)
        let mut command_bytes = [0u8; 12];
        command_bytes[..command.len()].copy_from_slice(command.as_bytes());
        msg.extend_from_slice(&command_bytes);
        // Payload size
        let payload_bytes = bitcoin::consensus::encode::serialize(payload);
        msg.extend_from_slice(&(payload_bytes.len() as u32).to_le_bytes());
        // Checksum
        let checksum = bitcoin::hashes::sha256d::Hash::hash(&payload_bytes)[..4].to_vec();
        msg.extend_from_slice(&checksum);
        // Payload
        msg.extend_from_slice(&payload_bytes);

        self.stream.write_all(&msg).await?;
        Ok(())
    }

    async fn receive_message(&mut self) -> Result<(String, Vec<u8>)> {
        let mut header = [0u8; 24];
        self.stream.read_exact(&mut header).await?;

        let command = String::from_utf8(header[4..16].to_vec())?
            .trim_matches(char::from(0))
            .to_string();
        let payload_len = u32::from_le_bytes(header[16..20].try_into()?);

        let mut payload = vec![0u8; payload_len as usize];
        self.stream.read_exact(&mut payload).await?;

        Ok((command, payload))
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct NetAddress {
    services: u64,
    address: std::net::IpAddr,
    port: u16,
}

impl Default for NetAddress {
    fn default() -> Self {
        Self {
            services: 0,
            address: std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            port: 0,
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct VersionMessage {
    version: i32,
    services: u64,
    timestamp: i64,
    addr_recv: NetAddress,
    addr_from: NetAddress,
    nonce: u64,
    user_agent: String,
    start_height: i32,
    relay: bool,
}
#[derive(Debug)]
struct EmptyMessage;

impl Encodable for EmptyMessage {
    fn consensus_encode<W: Write + ?Sized>(&self, _: &mut W) -> Result<usize, bitcoin::io::Error> {
        Ok(0)
    }
}

impl Encodable for VersionMessage {
    fn consensus_encode<W: Write + ?Sized>(
        &self,
        writer: &mut W,
    ) -> Result<usize, bitcoin::io::Error> {
        let mut len = 0;
        len += self.version.consensus_encode(writer)?;
        len += self.services.consensus_encode(writer)?;
        len += self.timestamp.consensus_encode(writer)?;
        // Skip addr_recv/from for simplicity
        len += 0u64.consensus_encode(writer)?; // addr_recv services
        len += [0u8; 16].consensus_encode(writer)?; // addr_recv IP
        len += 0u16.consensus_encode(writer)?; // addr_recv port
        len += 0u64.consensus_encode(writer)?; // addr_from services
        len += [0u8; 16].consensus_encode(writer)?; // addr_from IP
        len += 0u16.consensus_encode(writer)?; // addr_from port
        len += self.nonce.consensus_encode(writer)?;
        len += bitcoin::consensus::encode::VarInt(self.user_agent.len() as u64)
            .consensus_encode(writer)?;
        len += writer.write(self.user_agent.as_bytes())?;
        len += self.start_height.consensus_encode(writer)?;
        len += self.relay.consensus_encode(writer)?;
        Ok(len)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MempoolTxInfo {
    tx: Transaction,
    fee: u64,
    vsize: u64,
    fee_rate: f64,
    ancestors: HashSet<Txid>,
    descendants: HashSet<Txid>,
    received_time: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockTemplate {
    txids: Vec<Txid>,
    total_fees: u64,
    total_vsize: u64,
    expected_reward: u64,
    timestamp: SystemTime,
}

#[derive(Clone)]
struct MempoolTracker {
    daemon_url: String,
    auth: Option<String>,
    p2p_addr: Option<String>,
    mempool_txs: Arc<RwLock<HashMap<Txid, MempoolTxInfo>>>,
    block_templates: Arc<RwLock<Vec<BlockTemplate>>>,
    last_template_update: Arc<RwLock<SystemTime>>,
}

#[derive(Clone)]
struct AppState {
    tracker: Arc<MempoolTracker>,
}

#[derive(Deserialize, Serialize)]
struct JsonRpcRequest {
    id: u32,
    method: String,
    params: Vec<Value>,
    jsonrpc: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FeeEstimate {
    target_blocks: u32,
    fee_rate: f64, // sat/vB
}
#[derive(Deserialize, Serialize)]
struct JsonRpcResponse {
    id: u32,
    result: Value,
    jsonrpc: String,
}

#[derive(Deserialize, Serialize)]
struct JsonRpcError {
    id: u32,
    error: JsonRpcErrorObject,
    jsonrpc: String,
}

#[derive(Deserialize, Serialize)]
struct JsonRpcErrorObject {
    code: i32,
    message: String,
    data: Option<String>,
}

#[allow(dead_code)]
impl MempoolTracker {
    pub fn new(daemon_url: String, auth: Option<String>, p2p_addr: Option<String>) -> Self {
        MempoolTracker {
            daemon_url,
            auth,
            p2p_addr,
            mempool_txs: Arc::new(RwLock::new(HashMap::new())),
            block_templates: Arc::new(RwLock::new(Vec::new())),
            last_template_update: Arc::new(RwLock::new(SystemTime::UNIX_EPOCH)),
        }
    }
    fn build_block_header(&self) -> Vec<u8> {
        let mut header = Vec::with_capacity(BLOCK_HEADER_SIZE);

        // Version
        header.extend(&VERSION.to_le_bytes());

        // Previous block hash (zeros)
        header.extend(&[0u8; 32]);

        // Merkle root (will be updated later)
        header.extend(&[0u8; 32]);

        // Timestamp (current time)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs() as u32;
        header.extend(&timestamp.to_le_bytes());

        // Bits (difficulty target - using testnet value)
        header.extend(&0x1d00ffffu32.to_le_bytes());

        // Nonce
        header.extend(&0u32.to_le_bytes());

        header
    }

    fn encode_block(
        &self,
        template: &BlockTemplate,
        txs: &HashMap<Txid, MempoolTxInfo>,
    ) -> Result<String> {
        let mut block_data = self.build_block_header();

        // Add transaction count as varint
        let tx_count = template.txids.len() as u64 + 1; // +1 for coinbase
        let mut varint = Vec::new();
        bitcoin::consensus::encode::VarInt(tx_count).consensus_encode(&mut varint)?;
        block_data.extend(varint);

        // Add dummy coinbase transaction
        let coinbase_tx = Transaction {
            version: bitcoin::transaction::Version(1),
            lock_time: LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: bitcoin::OutPoint::default(),
                script_sig: bitcoin::Script::new().into(),
                sequence: bitcoin::Sequence(0xFFFFFFFF),
                witness: bitcoin::Witness::default(),
            }],
            output: vec![bitcoin::TxOut {
                value: Amount::from_sat(template.expected_reward),
                script_pubkey: bitcoin::Script::new().into(),
            }],
        };
        block_data.extend(bitcoin::consensus::encode::serialize(&coinbase_tx));

        // Add all other transactions
        for txid in &template.txids {
            if let Some(tx_info) = txs.get(txid) {
                block_data.extend(bitcoin::consensus::encode::serialize(&tx_info.tx));
            }
        }

        Ok(hex::encode(block_data))
    }

    async fn make_rpc_call(&self, method: &str, params: Vec<Value>) -> Result<Value> {
        let start_time = SystemTime::now();
        debug!(
            "Making RPC call to node - method: {}, params: {:?}",
            method, params
        );

        let client = reqwest::Client::new();
        let url = match self.auth.clone() {
            Some(auth) => {
                let mut url = Url::parse(&self.daemon_url)?;
                let (username, password) = auth
                    .split(":")
                    .next_tuple()
                    .ok_or_else(|| anyhow!("Invalid auth format - should be username:password"))?;
                url.set_username(username)
                    .map_err(|_| anyhow!("Invalid username"))?;
                url.set_password(Some(password))
                    .map_err(|_| anyhow!("Invalid password"))?;
                url
            }
            None => Url::parse(&self.daemon_url)?,
        };

        let response = client
            .post(url)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params
            }))
            .send()
            .await?
            .json::<Value>()
            .await?;

        let duration = SystemTime::now()
            .duration_since(start_time)
            .unwrap_or(Duration::from_secs(0));

        if let Some(error) = response.get("error") {
            info!(
                "RPC call failed after {:.2}s - method: {}, error: {}",
                duration.as_secs_f64(),
                method,
                error
            );
            return Err(anyhow!("RPC error: {}", error));
        }

        debug!(
            "RPC call succeeded in {:.2}s - method: {}",
            duration.as_secs_f64(),
            method
        );

        Ok(response["result"].clone())
    }
    async fn update_mempool_p2p(&self, p2p_addr: &str) -> Result<()> {
        let mut conn = P2PConnection::new(p2p_addr).await?;

        // Handshake
        conn.send_version().await?;

        while !conn.version_received || !conn.verack_received {
            let (command, _payload) = conn.receive_message().await?;
            match command.as_str() {
                "version" => {
                    conn.version_received = true;
                    conn.send_message("verack", &EmptyMessage).await?;
                }
                "verack" => {
                    conn.verack_received = true;
                }
                _ => continue,
            }
        }

        // Request mempool contents
        conn.send_message("mempool", &EmptyMessage).await?;

        let start_time = SystemTime::now();
        let mut new_txs = HashSet::new();

        // Process incoming inventory messages
        loop {
            let (command, payload) = conn.receive_message().await?;
            match command.as_str() {
                "inv" => {
                    let count =
                        bitcoin::consensus::encode::VarInt::consensus_decode(&mut &payload[..])?;
                    let mut offset = bitcoin::consensus::encode::VarInt(count.0)
                        .consensus_encode(&mut vec![])?;

                    let mut getdata_items = Vec::new();

                    for _ in 0..count.0 {
                        let inv_type = u32::consensus_decode(&mut &payload[offset..])?;
                        offset += 4;

                        if inv_type == 1 {
                            // MSG_TX
                            let mut hash = [0u8; 32];
                            hash.copy_from_slice(&payload[offset..offset + 32]);
                            offset += 32;

                            let txid = Txid::from_byte_array(hash.into());
                            if !self.mempool_txs.read().await.contains_key(&txid) {
                                getdata_items.push(Inventory::Block(BlockHash::from_byte_array(
                                    hash.into(),
                                )));
                                new_txs.insert(txid);
                            }
                        }
                    }

                    if !getdata_items.is_empty() {
                        // Request transactions we don't have
                        conn.send_message("getdata", &getdata_items).await?;
                    }
                }
                "tx" => {
                    let tx: Transaction = bitcoin::consensus::encode::deserialize(&payload)?;
                    let txid = tx.compute_txid();

                    if new_txs.contains(&txid) {
                        let mut txs = self.mempool_txs.write().await;

                        // Calculate basic fee and size info
                        let vsize = u64::from(tx.weight()) / 4;
                        // For now estimate fee based on typical rate - in production would validate inputs
                        let fee = vsize * 2; // Assume 2 sat/vB fee rate

                        txs.insert(
                            txid,
                            MempoolTxInfo {
                                tx,
                                fee,
                                vsize,
                                fee_rate: fee as f64 / vsize as f64,
                                ancestors: HashSet::new(), // Will be updated in post-processing
                                descendants: HashSet::new(),
                                received_time: SystemTime::now(),
                            },
                        );

                        new_txs.remove(&txid);
                    }
                }
                _ => continue,
            }

            // Break if we've received all transactions or hit timeout
            if new_txs.is_empty() || start_time.elapsed()? > Duration::from_secs(30) {
                break;
            }
        }

        // Update ancestor/descendant relationships
        let txs = self.mempool_txs.read().await;
        let txids: Vec<_> = txs.keys().cloned().collect();
        drop(txs); // Release read lock

        let mut txs = self.mempool_txs.write().await;
        for txid in txids {
            if let Some(tx) = txs.get(&txid) {
                let ancestors: HashSet<_> = tx
                    .tx
                    .input
                    .iter()
                    .filter_map(|input| {
                        if txs.contains_key(&input.previous_output.txid) {
                            Some(input.previous_output.txid)
                        } else {
                            None
                        }
                    })
                    .collect();

                if !ancestors.is_empty() {
                    if let Some(tx) = txs.get_mut(&txid) {
                        tx.ancestors = ancestors.clone();
                    }

                    for ancestor_txid in ancestors {
                        if let Some(ancestor_tx) = txs.get_mut(&ancestor_txid) {
                            ancestor_tx.descendants.insert(txid);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn update_mempool(&self) -> Result<()> {
        let start_time = SystemTime::now();
        info!("Starting mempool sync");

        // First try p2p connection if configured
        if let Some(p2p_addr) = &self.p2p_addr {
            if let Err(e) = self.update_mempool_p2p(p2p_addr).await {
                debug!("P2P mempool update failed: {}", e);
            }
        }

        let mempool_info = self
            .make_rpc_call("getrawmempool", vec![json!(true)])
            .await?;

        let mut txs = self.mempool_txs.write().await;
        let initial_count = txs.len();

        // Remove stale transactions
        txs.retain(|txid, _| mempool_info[txid.to_string()].is_object());
        let removed_count = initial_count - txs.len();
        if removed_count > 0 {
            info!("Removed {} stale transactions", removed_count);
        }

        // Add/update transactions
        let mut added_count = 0;
        for (txid_str, info) in mempool_info.as_object().unwrap() {
            let txid = Txid::from_hex(txid_str)?;

            if !txs.contains_key(&txid) {
                let tx_hex = self
                    .make_rpc_call("getrawtransaction", vec![json!(txid_str)])
                    .await?;
                let tx_bytes = hex::decode(tx_hex.as_str().unwrap())?;
                let tx: Transaction = deserialize(&tx_bytes)?;

                txs.insert(
                    txid,
                    MempoolTxInfo {
                        tx,
                        fee: (info["fees"]["base"].as_f64().unwrap() * 100_000_000.0) as u64,
                        vsize: info["vsize"].as_u64().unwrap(),
                        fee_rate: info["fees"]["base"].as_f64().unwrap() * 100_000_000.0
                            / info["vsize"].as_f64().unwrap(),
                        ancestors: info["depends"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .filter_map(|v| Txid::from_hex(v.as_str()?).ok())
                            .collect(),
                        descendants: HashSet::new(),
                        received_time: SystemTime::now(),
                    },
                );
                added_count += 1;
            }
        }

        if added_count > 0 {
            info!("Added {} new transactions", added_count);
        }

        // Update descendant sets
        let txids: Vec<_> = txs.keys().cloned().collect();
        for txid in txids {
            let ancestors = txs.get(&txid).unwrap().ancestors.clone();
            for ancestor_txid in ancestors {
                if let Some(ancestor_tx) = txs.get_mut(&ancestor_txid) {
                    ancestor_tx.descendants.insert(txid);
                }
            }
        }

        let duration = SystemTime::now()
            .duration_since(start_time)
            .unwrap_or(Duration::from_secs(0));
        info!(
            "Mempool sync complete - {} total transactions in {:.2}s",
            txs.len(),
            duration.as_secs_f64()
        );

        Ok(())
    }

    fn get_ancestor_set_info(
        &self,
        txs: &HashMap<Txid, MempoolTxInfo>,
        txid: &Txid,
    ) -> (u64, u64, f64) {
        let mut total_fee = 0;
        let mut total_vsize = 0;
        let mut seen = HashSet::new();
        let mut queue = vec![txid];

        while let Some(current_txid) = queue.pop() {
            if !seen.insert(*current_txid) {
                continue;
            }

            if let Some(tx_info) = txs.get(current_txid) {
                total_fee += tx_info.fee;
                total_vsize += tx_info.vsize;
                queue.extend(tx_info.ancestors.iter());
            }
        }

        let fee_rate = if total_vsize > 0 {
            total_fee as f64 / total_vsize as f64
        } else {
            0.0
        };

        (total_fee, total_vsize, fee_rate)
    }

    async fn generate_block_templates(&self) -> Result<()> {
        let txs = self.mempool_txs.read().await;
        let mut templates = Vec::new();

        // Basic high-fee template
        let mut tx_entries: Vec<_> = txs.iter().collect();
        tx_entries.sort_by(|a, b| {
            b.1.fee_rate
                .partial_cmp(&a.1.fee_rate)
                .unwrap_or(Ordering::Equal)
        });

        let mut template = BlockTemplate {
            txids: Vec::new(),
            total_fees: 0,
            total_vsize: 0,
            expected_reward: 6_25 * 100_000_000, // Current block reward in sats
            timestamp: SystemTime::now(),
        };

        let mut included = HashSet::new();
        for (txid, info) in tx_entries {
            if info.fee_rate < MIN_FEE_RATE || included.contains(txid) {
                continue;
            }

            let new_vsize = template.total_vsize + info.vsize;
            if new_vsize > MAX_BLOCK_WEIGHT as u64 {
                continue;
            }

            // Include transaction and its ancestors
            let mut queue = vec![txid];
            while let Some(current_txid) = queue.pop() {
                if included.insert(*current_txid) {
                    if let Some(tx_info) = txs.get(current_txid) {
                        template.txids.push(*current_txid);
                        template.total_fees += tx_info.fee;
                        template.total_vsize += tx_info.vsize;
                        queue.extend(tx_info.ancestors.iter());
                    }
                }
            }
        }

        template.expected_reward += template.total_fees;
        templates.push(template);

        // Store templates
        let mut block_templates = self.block_templates.write().await;
        *block_templates = templates;

        let mut last_update = self.last_template_update.write().await;
        *last_update = SystemTime::now();

        Ok(())
    }

    pub async fn start_background_tasks(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                if let Err(e) = self.update_mempool().await {
                    debug!("Error updating mempool: {}", e);
                }

                // Check if templates need updating
                let last_update = *self.last_template_update.read().await;
                if SystemTime::now()
                    .duration_since(last_update)
                    .unwrap_or(Duration::from_secs(0))
                    >= TEMPLATE_CACHE_DURATION
                {
                    if let Err(e) = self.generate_block_templates().await {
                        debug!("Error generating block templates: {}", e);
                    }
                }

                sleep(UPDATE_INTERVAL).await;
            }
        });
    }
}

#[post("/")]
async fn handle_jsonrpc(
    body: web::Json<JsonRpcRequest>,
    state: web::Data<AppState>,
) -> ActixResult<impl Responder> {
    debug!(
        "Received JSON-RPC request - method: {}, params: {:?}",
        body.method, body.params
    );

    match body.method.as_str() {
        "memshrew_build" => {
            let txs = state.tracker.mempool_txs.read().await;
            let templates = state.tracker.block_templates.read().await;

            let mut blocks = Vec::new();
            for template in templates.iter() {
                match state.tracker.encode_block(template, &txs) {
                    Ok(hex) => blocks.push(hex),
                    Err(e) => debug!("Error encoding block: {}", e),
                }
            }

            Ok(HttpResponse::Ok().json(JsonRpcResponse {
                id: body.id,
                result: json!(blocks),
                jsonrpc: "2.0".to_string(),
            }))
        }
        "memshrew_getmempooltxs" => {
            let txs = state.tracker.mempool_txs.read().await;
            let result = txs.iter()
                    .map(|(txid, info)| json!({
                        "txid": txid.to_string(),
                        "fee": info.fee,
                        "vsize": info.vsize,
                        "fee_rate": info.fee_rate,
                        "ancestors": info.ancestors.iter().map(|tx| tx.to_string()).collect::<Vec<_>>(),
                        "descendants": info.descendants.iter().map(|tx| tx.to_string()).collect::<Vec<_>>(),
                    }))
                    .collect::<Vec<_>>();

            Ok(HttpResponse::Ok().json(JsonRpcResponse {
                id: body.id,
                result: json!(result),
                jsonrpc: "2.0".to_string(),
            }))
        }

        "memshrew_getblocktemplates" => {
            let templates = state.tracker.block_templates.read().await;
            let result = templates
                .iter()
                .map(|template| {
                    json!({
                        "txids": template.txids.iter().map(|tx| tx.to_string()).collect::<Vec<_>>(),
                        "total_fees": template.total_fees,
                        "total_vsize": template.total_vsize,
                        "expected_reward": template.expected_reward,
                        "timestamp": template.timestamp
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or(Duration::from_secs(0))
                            .as_secs(),
                    })
                })
                .collect::<Vec<_>>();

            Ok(HttpResponse::Ok().json(JsonRpcResponse {
                id: body.id,
                result: json!(result),
                jsonrpc: "2.0".to_string(),
            }))
        }

        "memshrew_estimatefees" => {
            let txs = state.tracker.mempool_txs.read().await;
            let mut fee_rates: Vec<_> = txs.iter().map(|(_, info)| info.fee_rate).collect();
            fee_rates.sort_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

            let estimates: Vec<FeeEstimate> = BLOCK_CONFIRM_TARGETS
                .iter()
                .map(|&target| {
                    // Use higher percentiles for lower confirmation targets
                    let percentile = match target {
                        1 => 0.05, // 95th percentile for 1-block
                        2 => 0.20, // 80th percentile for 2-blocks
                        3 => 0.50, // 50th percentile for 3-blocks
                        _ => 0.75, // Fallback, shouldn't happen
                    };

                    let index = ((fee_rates.len() as f64) * percentile) as usize;
                    let fee_rate = fee_rates.get(index).copied().unwrap_or(MIN_FEE_RATE);

                    FeeEstimate {
                        target_blocks: target,
                        fee_rate: fee_rate.max(MIN_FEE_RATE), // Never go below min fee rate
                    }
                })
                .collect();

            Ok(HttpResponse::Ok().json(JsonRpcResponse {
                id: body.id,
                result: json!(estimates),
                jsonrpc: "2.0".to_string(),
            }))
        }

        _ => Ok(HttpResponse::Ok().json(JsonRpcError {
            id: body.id,
            error: JsonRpcErrorObject {
                code: -32601,
                message: format!("Method '{}' not found", body.method),
                data: None,
            },
            jsonrpc: "2.0".to_string(),
        })),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Create mempool tracker
    let tracker = Arc::new(MempoolTracker::new(
        args.daemon_rpc_url,
        args.auth,
        args.p2p_addr,
    ));

    // Start background tasks
    tracker.clone().start_background_tasks().await;

    // Create app state
    let app_state = web::Data::new(AppState {
        tracker: tracker.clone(),
    });

    // Start the JSON-RPC server
    info!("Starting server at http://{}:{}", args.host, args.port);
    HttpServer::new(move || {
        App::new()
            .wrap(Cors::default().allowed_origin_fn(|origin, _| {
                if let Ok(origin_str) = origin.to_str() {
                    origin_str.starts_with("http://localhost:")
                } else {
                    false
                }
            }))
            .app_data(app_state.clone())
            .service(handle_jsonrpc)
    })
    .bind((args.host.as_str(), args.port))?
    .run()
    .await?;

    Ok(())
}
