use crate::environment::RuntimeEnvironment;
use crate::proto::metashrew::KeyValueFlush;
use protobuf::Message;
use std::sync::Arc;

pub fn get<E: RuntimeEnvironment>(env: &mut E, v: Arc<Vec<u8>>) -> Arc<Vec<u8>> {
    if env.cache().contains_key(&v.clone()) {
        return env.cache().get(&v.clone()).unwrap().clone();
    }
    let value = env.get(v.as_ref()).map_or(vec![], |v| v);
    let value = Arc::new(value);
    env.cache().insert(v.clone(), value.clone());
    value
}

pub fn set<E: RuntimeEnvironment>(env: &mut E, k: Arc<Vec<u8>>, v: Arc<Vec<u8>>) {
    env.cache().insert(k.clone(), v.clone());
    env.to_flush().push(k.clone());
}

pub fn flush<E: RuntimeEnvironment>(env: &mut E) {
    let mut to_encode: Vec<Vec<u8>> = Vec::<Vec<u8>>::new();
    for item in env.to_flush().clone().iter() {
        to_encode.push((*item.clone()).clone());
        to_encode.push((*(env.cache().get(item).unwrap().clone())).clone());
    }
    env.to_flush().clear();
    let mut buffer = KeyValueFlush::new();
    buffer.list = to_encode;
    let serialized = buffer.write_to_bytes().unwrap();
    env.flush(&serialized).unwrap();
}

pub fn clear<E: RuntimeEnvironment>(env: &mut E) {
    env.to_flush().clear();
    env.cache().clear();
}
