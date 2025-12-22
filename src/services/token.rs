use uuid::Uuid;

pub fn generate_verification_token() -> String {
    Uuid::new_v4().to_string()
}
