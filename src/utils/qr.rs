use std::io::Cursor;

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, SecondsFormat, Utc};
use image::{ImageFormat, Luma};
use qrcode::QrCode;
use serde::Serialize;
use uuid::Uuid;

use crate::errors::AppError;

/// Same payload the React `generateEventQRCode` util produced, so the mobile
/// scanner keeps working unchanged. (Field order matches the TypeScript object.)
#[derive(Serialize)]
struct QrPayload {
    #[serde(rename = "eventId")]
    event_id: Uuid,
    #[serde(rename = "type")]
    kind: &'static str,
    timestamp: String,
}

/// Returns a `data:image/png;base64,...` URL of a 256px QR code, like `QRCode.toDataURL`.
pub fn event_qr_data_url(event_id: Uuid, at: DateTime<Utc>) -> Result<String, AppError> {
    let payload = serde_json::to_string(&QrPayload {
        event_id,
        kind: "event_attendance",
        timestamp: at.to_rfc3339_opts(SecondsFormat::Millis, true), // JS toISOString() format
    })
    .map_err(|e| AppError::Internal(format!("qr payload: {e}")))?;

    let code = QrCode::new(payload.as_bytes())
        .map_err(|e| AppError::Internal(format!("qr encode: {e}")))?;
    let img = code.render::<Luma<u8>>().min_dimensions(256, 256).quiet_zone(true).build();

    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("qr png: {e}")))?;

    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decodes the generated image back and checks the payload is what the app expects.
    #[test]
    fn qr_round_trips_to_the_expected_payload() {
        let id = Uuid::new_v4();
        let url = event_qr_data_url(id, Utc::now()).unwrap();
        let b64 = url.strip_prefix("data:image/png;base64,").expect("png data url");
        let bytes = STANDARD.decode(b64).unwrap();

        let gray = image::load_from_memory(&bytes).unwrap().to_luma8();
        assert!(gray.width() >= 256);

        let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(
            gray.width() as usize,
            gray.height() as usize,
            |x, y| gray.get_pixel(x as u32, y as u32).0[0],
        );
        let grids = prepared.detect_grids();
        assert_eq!(grids.len(), 1, "expected exactly one QR code");
        let (_meta, content) = grids[0].decode().unwrap();

        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["eventId"], id.to_string());
        assert_eq!(json["type"], "event_attendance");
        assert!(json["timestamp"].as_str().unwrap().ends_with('Z'));
    }
}
