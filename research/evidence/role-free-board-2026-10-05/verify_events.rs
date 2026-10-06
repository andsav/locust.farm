// Compile against the experiment's locust_proto and serde_json build artifacts.
// Decode rechecks canonical bytes, structure, and Ed25519 signature for each event.
use locust_proto::{event::Event, id::Signature};
use serde_json::{Value, json};
fn unhex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect()
}
fn main() {
    let arg = std::env::args().nth(1).expect("signed JSON input");
    let rows: Vec<Value> = serde_json::from_slice(&std::fs::read(arg).unwrap()).unwrap();
    let mut output = Vec::new();
    for row in rows {
        let bytes = unhex(row["header_hex"].as_str().unwrap());
        let sig = Signature(
            unhex(row["signature_hex"].as_str().unwrap())
                .try_into()
                .unwrap(),
        );
        let event = Event::decode(&bytes, sig).expect("signature/canonical validation");
        assert_eq!(event.id().to_string(), row["id"].as_str().unwrap());
        assert_eq!(
            event.header().author.to_string(),
            row["author"].as_str().unwrap()
        );
        assert_eq!(event.header().seq, row["seq"].as_u64().unwrap());
        output.push(json!({"id":event.id(),"position":row["position"],"header":event.header(),"signature_verified":true}));
    }
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
