use std::path::Path;

use alloy::rpc::types::engine::JwtSecret;

const DEFAULT_JWT_SECRET_NAME: &str = "jwt.hex";

pub fn read_default_jwt_secret(data_dir: &Path) -> anyhow::Result<JwtSecret> {
    let default_jwt_secret_path = data_dir.join(DEFAULT_JWT_SECRET_NAME);
    match default_jwt_secret_path.is_file() {
        true => Ok(JwtSecret::from_file(&default_jwt_secret_path)?),
        false => Ok(JwtSecret::try_create_random(&default_jwt_secret_path)?),
    }
}
