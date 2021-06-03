
use uuid::Uuid;

use siderite_core::auth::*;

pub struct MojangAuthenticator;

impl Authenticator for MojangAuthenticator {
    fn authenticate(&self, info: AuthInfo) -> Result {
        if info.server_id.is_none() {
            return Err(Error::NoServerId);
        }

        // TODO: handle errors?
        let res = mojang::auth_with_yggdrasil(&info.username, &info.server_id.unwrap()).unwrap();
        let uuid = Uuid::parse_str(&res.id).unwrap();

        Ok(AuthResponse {
            client_id: info.client_id,
            username: res.name,
            uuid,
            properties: res.properties
        })
    }
}
