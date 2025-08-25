#[allow(unused_imports)]
use crate::{
    println,
    stdio::{stdout, Write},
};
use crate::{runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_support::{
    cellpack::Cellpack,
    constants::AUTH_TOKEN_FACTORY_ID,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    utils::string_to_u128_list,
};
use anyhow::{anyhow, Result};
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

pub trait AuthenticatedResponder: AlkaneResponder {
    fn auth_token_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/auth")
    }
    fn set_auth_token(&self, auth_token: AlkaneId) -> Result<()> {
        let mut ptr = self.auth_token_pointer();
        if ptr.get().len() == 0 {
            ptr.set(Arc::new(<AlkaneId as Into<Vec<u8>>>::into(auth_token)));
            Ok(())
        } else {
            Err(anyhow!("auth token already set"))
        }
    }
    fn deploy_auth_token_name_symbol(
        &self,
        name: String,
        symbol: String,
        units: u128,
    ) -> Result<AlkaneTransfer> {
        let mut input = vec![0];
        input.extend(string_to_u128_list(name));
        input.extend(string_to_u128_list(symbol));
        input.push(units);
        let cellpack = Cellpack {
            target: AlkaneId {
                block: 6,
                tx: AUTH_TOKEN_FACTORY_ID,
            },
            inputs: input,
        };
        let sequence = self.sequence();
        let response = self.call(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;
        self.set_auth_token(AlkaneId {
            block: 2,
            tx: sequence,
        })?;
        if response.alkanes.0.len() < 1 {
            Err(anyhow!("auth token not returned with factory"))
        } else {
            Ok(response.alkanes.0[0])
        }
    }
    fn deploy_auth_token(&self, units: u128) -> Result<AlkaneTransfer> {
        let context = self.context()?;
        self.deploy_auth_token_name_symbol(
            format!("AUTH {:?}", context.myself),
            format!("AUTH {:?}", context.myself),
            units,
        )
    }
    // self auth uses the same contract alkane id as the auth token, avoids needing to deploy and have to manage a separate token
    fn deploy_self_auth_token(&self, units: u128) -> Result<AlkaneTransfer> {
        let context = self.context()?;
        self.set_auth_token(context.myself)?;
        Ok(AlkaneTransfer {
            id: context.myself,
            value: units,
        })
    }
    fn auth_token(&self) -> Result<AlkaneId> {
        let pointer = self.auth_token_pointer().get();
        Ok(pointer.as_ref().clone().try_into()?)
    }
    fn only_owner(&self) -> Result<()> {
        let context = self.context()?;
        let auth_token = self.auth_token()?;
        if !context
            .incoming_alkanes
            .0
            .iter()
            .any(|i| (i.id == auth_token && i.value > 0))
        {
            return Err(anyhow!("Auth token is not in incoming alkanes"));
        }
        if auth_token == context.myself {
            return Ok(());
        }
        let cellpack = Cellpack {
            target: auth_token,
            inputs: vec![0x1],
        };
        let response = self.call(
            &cellpack,
            &AlkaneTransferParcel(vec![AlkaneTransfer {
                id: cellpack.target.clone(),
                value: 1,
            }]),
            self.fuel(),
        )?;
        if response.data == vec![0x01] {
            Ok(())
        } else {
            Err(anyhow!("only_owner: returned error"))
        }
    }
}
