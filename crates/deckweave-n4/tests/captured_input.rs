use deckweave_n4::{Input, decode};
use serde_json::Value;

#[test]
fn owner_physical_key_and_touch_packets_replay_without_using_output_numbering() {
    let capture: Value = serde_json::from_str(include_str!("fixtures/owner-1021.json")).unwrap();
    for case in capture["keys"].as_array().unwrap() {
        let bytes: Vec<u8> = case["raw"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect();
        assert_eq!(
            decode(&bytes),
            Some(Input::Key {
                key: case["physical_key"].as_u64().unwrap() as u8,
                pressed: case["pressed"].as_bool().unwrap()
            })
        );
    }
    for (index, case) in capture["touch"].as_array().unwrap().iter().enumerate() {
        let bytes: Vec<u8> = case["raw"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect();
        assert_eq!(
            decode(&bytes),
            Some(Input::Touch {
                x: 101,
                pressed: index == 0
            })
        );
    }
}
