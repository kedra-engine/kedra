//! Compact encoded payloads keep exact bytes and admit legacy deliveries.
#![allow(clippy::unwrap_used, reason = "Image wire contract assertions")]
use super::images::*;

#[test]
fn compact_image_payload_roundtrips_without_numeric_byte_inflation() {
    let png: Vec<u8> = (0..=255).cycle().take(4096).collect();
    let package = ImageVariantsDelivery {
        format: 2,
        logical_size: [1, 1],
        variants: vec![ImageVariant {
            density: 1,
            png: png.clone(),
        }],
    };
    let encoded = ron::ser::to_string(&package).unwrap();
    assert!(
        encoded.len() < 5700,
        "Image bytes were expanded into numeric RON: {}",
        encoded.len()
    );
    let decoded: ImageVariantsDelivery = ron::from_str(&encoded).unwrap();
    assert_eq!(decoded.variants[0].png, png);
}

#[test]
fn legacy_numeric_payloads_remain_readable_and_invalid_payloads_are_rejected() {
    let package: ImageVariantsDelivery =
        ron::from_str("(format:1,logical_size:(1,1),variants:[(density:1,png:[137,80,78,71])])")
            .unwrap();
    assert_eq!(package.variants[0].png, [137, 80, 78, 71]);
    for png in ["\"not base64!\"", "[256]", "[1,-1]"] {
        assert!(
            ron::from_str::<ImageVariantsDelivery>(&format!(
                "(format:2,logical_size:(1,1),variants:[(density:1,png:{png})])"
            ))
            .is_err()
        );
    }
}
