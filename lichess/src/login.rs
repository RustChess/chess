use url::Url;

use chess_lichess_client::types;

pub const CLIENT_ID: &str = "github.com/RustChess/rustchess";
pub const URL: &str = "https://lichess.org";

#[derive(Clone, Debug)]
pub struct Login;

#[derive(Debug, thiserror::Error)]
#[error("login error")]
pub struct Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

impl Login {
    // e.g. Some("engine:read engine:write")
    pub async fn new(scope: Option<&str>) -> Result<Self> {
        let (challenge, verifier) = oauth2::PkceCodeChallenge::new_random_sha256();
        let _ = verifier;
        // let client = crate::Client::new(URL);

        let redirect_uri = "http://127.0.0.1:8765";
        let state = "random-state";

        let mut url = Url::parse(&format!("{URL}/oauth")).expect("valid URL");
        url.query_pairs_mut()
            .append_pair("response_type", &types::OauthResponseType::Code.to_string())
            .append_pair("client_id", CLIENT_ID)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("code_challenge", challenge.as_str())
            .append_pair(
                "code_challenge_method",
                &types::OauthCodeChallengeMethod::S256.to_string(),
            )
            .append_pair("state", state);
        if let Some(scope) = scope {
            url.query_pairs_mut().append_pair("scope", scope);
        }
        println!("{url}");

        // let username = None;
        // let result = client.oauth(
        //     CLIENT_ID,
        //     challenge.as_str(),
        //     types::OauthCodeChallengeMethod::S256,
        //     "http://localhost:8765",
        //     types::OauthResponseType::Code,
        //     scope,
        //     Some(state),
        //     username,
        // ).await;

        // let url = format!("
        // let redirect_url = format!("
        todo!();
    }
}

// #[cfg(test)]
// #[tokio::test]
// async fn login() {
//     // e.g. Some("engine:read engine:write")
//     Login::new(Some("engine:read engine:write")).await.unwrap();
//     panic!("wtf");
// }
