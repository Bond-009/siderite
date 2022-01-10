#![forbid(unsafe_code)]

use async_trait::async_trait;
use mojang::MojangClient;
use uuid::Uuid;

use siderite_core::auth::*;

pub struct MojangAuthenticator {
    client: MojangClient
}

impl MojangAuthenticator {
    pub fn new() -> Self {
        Self {
            client: MojangClient::new()
        }
    }
}

#[async_trait]
impl Authenticator for MojangAuthenticator {
    async fn authenticate(&self, info: AuthInfo) -> Result {
        if info.server_id.is_none() {
            return Err(Error::NoServerId);
        }

        let res = self.client.auth_with_yggdrasil(&info.username, &info.server_id.unwrap()).await.map_err(|_| Error::Failed)?;
        let uuid = Uuid::parse_str(&res.id).unwrap();

        Ok(AuthResponse {
            client_id: info.client_id,
            username: res.name,
            uuid,
            properties: res.properties
        })
    }
}
