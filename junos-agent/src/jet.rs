use std::{fs, marker::PhantomData, path::PathBuf};

use anyhow::{anyhow, bail, Context};

use jet::junos_21_4::jnx::jet::{
    authentication::{authentication_client::AuthenticationClient, LoginRequest},
    common::StatusCode,
    management::{
        management_client::ManagementClient, op_command_get_request::Command, OpCommandGetRequest,
        OpCommandOutputFormat,
    },
};

use tokio::net::UnixStream;

use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint};

pub(crate) trait State {}

pub(crate) enum New {}
impl State for New {}

pub(crate) enum Authenticated {}
impl State for Authenticated {}

pub(crate) enum Transport {
    Unix(UnixTransport),
    Https(HttpsTransport),
}

impl Transport {
    pub(crate) const fn unix(path: PathBuf) -> Self {
        Self::Unix(UnixTransport::new(path))
    }

    pub(crate) fn https(
        host: String,
        port: u16,
        ca_cert_path: Option<PathBuf>,
        tls_server_name: Option<String>,
    ) -> anyhow::Result<Self> {
        HttpsTransport::new(host, port, ca_cert_path, tls_server_name).map(Self::Https)
    }

    pub(crate) async fn connect(self) -> anyhow::Result<Client<New>> {
        match self {
            Self::Unix(transport) => transport.connect().await,
            Self::Https(transport) => transport.connect().await,
        }
    }
}

pub(crate) struct UnixTransport {
    path: PathBuf,
}

impl UnixTransport {
    const fn new(path: PathBuf) -> Self {
        Self { path }
    }

    async fn connect(self) -> anyhow::Result<Client<New>> {
        Endpoint::from_static("http://[::]")
            .connect_with_connector(tower::service_fn(move |_| {
                let path = self.path.clone();
                log::debug!("attempting to connect to socket '{}'", path.display());
                UnixStream::connect(path)
            }))
            .await
            .context("failed to connect gRPC channel")
            .map(Client::from)
    }
}

pub(crate) struct HttpsTransport {
    host: String,
    port: u16,
    tls_config: ClientTlsConfig,
}

impl HttpsTransport {
    const DEFAULT_CA_CERT: &str = r"
-----BEGIN CERTIFICATE-----
MIIE+zCCAuOgAwIBAgIUPGca+m6PygDD+Tk3ApM+6wfuBI8wDQYJKoZIhvcNAQEL
BQAwDTELMAkGA1UEAwwCY2EwHhcNMjMwNTE3MTMzMDEwWhcNMjQwNTE2MTMzMDEw
WjANMQswCQYDVQQDDAJjYTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIB
ANmhCKjEUYoFOfas2BSUnK4Dv43BJ5axb4ukbRJGeBdR0l+X8N5LbOrAG7szr7UL
z32mTqHFewO4GGoMsE4kq5KIZo5sM8KiMuZZ3DuOtqZYuGUfucL8nHPT44zjATeF
wOmC52Hd6to6k+rVjyjZFBEQ4XZt+nrmEk9z11xz+Xhw266VKSxKYxWCtNub39Pg
305a+mdTT8jzFsH1FbjafNSFLkmeSPa59DZRNKPvQcKm1aP/3MQSh2AKyss9oEXy
Z2rVWAjNXmm2TKDV5lAHaT6HUL1ACUWLdK2uEROgYEkeiZgB697FsJ6UgLNzoHwe
CNRfMj1WNLEZgfi1cBaPnZzviYFGkpISL34ZJ6vh8jQWRw4TEFb7mItmu7ComWmX
KiU687mjijgs8z8SUy8jMJFyOqItSLk7W+nZYamHKD59OyeqO0vt/YLafigZvDPl
DPKN111j/EWElcuHys3jW47cAPdZ9AN3BT2X3Kwnx8P2a7W3OR3VCxGxiTgQfCGH
bGstjR6ZcP4aI1LxZcML6ULB1xup5VPMWXNgOlQPilHcIEA+FNu6S9R+3TgTWdn1
KwjjvbLSmUwj0SOV3xtB+Y3otVIrhPJSB9afgxkgcAVeX4/PHuulGtnTnmirgHYy
QHPWdgt8vlbv03JPFJvYVdmf70hJMFKishNsJKhDMG7jAgMBAAGjUzBRMB0GA1Ud
DgQWBBSTplYiyziOOCkEBErV5qU+U8jQ4jAfBgNVHSMEGDAWgBSTplYiyziOOCkE
BErV5qU+U8jQ4jAPBgNVHRMBAf8EBTADAQH/MA0GCSqGSIb3DQEBCwUAA4ICAQDM
oaDhsTDw+3aeJDlUWfcmbGNHCZzWXdV9AawrDgy4BU2Ta1BVtRA1yK8z3NL+uMzR
4Ya2EIrPr9MAp4JENkBwPT8ZnNuOMKzxTeecIjqBKV4vu18qMdOxUc7u1oA9bkpD
U49C15d8MwCm71m4P2xVBVLWMdL4nmKZkgAnZdNLf0NKTLTyFTA02jtT8ilw+vMd
P8zreo8YdbQCnGHSdw6ocvmQglCjDw2udU77pW6ieMLt2ecnTqX+ZXikfc+IqxAC
FCDFHy5qYp7hpbTjmTwXTn9C2TK9uucGaGj8bpo1Nmsc4sE589TCxDU4n9XqVHhg
k97tBbMKT9Bif5zmo47h0xP+x9BN0UiymV23hueJWYW1zlP0r5oe888UOjoyM1Wz
Gs3TulCvLfXTjpfOLi+DRp4h30DQ4s2wDTz4D3gwqvupQL2uHgXiwvIN/VYDla/r
zj1SQVB8IEUmchXJfYG7Hmj68SUX2LilfgsyszzQkldin+Mh82RlAq7Qwzuvd95P
Ca1J8mu09TDXW5ngbV3ZIFGXpKFe+UhykAGIqi87CKOeQkWUGBEh608WTtqB3ibA
WLDujFvoWA21FAWlbaab7OYYIx2q52SoA8K/6UjB2i1wJ5p2iEQOmhZDPTp4vMT5
WGlCmPoVjoc51As4M6pWmolTpW/P8jN0t36O84Bnzg==
-----END CERTIFICATE-----
";

    fn new(
        host: String,
        port: u16,
        ca_cert_path: Option<PathBuf>,
        tls_server_name: Option<String>,
    ) -> anyhow::Result<Self> {
        log::debug!("reading CA certificate");
        let ca_cert = ca_cert_path
            .map(fs::read_to_string)
            .transpose()
            .context("failed to read CA certificate file")?
            .or_else(|| Some(Self::DEFAULT_CA_CERT.to_string()))
            .map(Certificate::from_pem)
            .ok_or_else(|| anyhow!("failed to parse CA certificate"))?;
        log::debug!("setting up TLS config");
        let tls_config = {
            let config = ClientTlsConfig::new().ca_certificate(ca_cert);
            if let Some(name) = tls_server_name {
                config.domain_name(name)
            } else {
                config
            }
        };
        Ok(Self {
            host,
            port,
            tls_config,
        })
    }

    async fn connect(self) -> anyhow::Result<Client<New>> {
        let uri = format!("https://{}:{}", self.host, self.port);
        Endpoint::from_shared(uri)
            .context("failed to parse endpoint URL")?
            .tls_config(self.tls_config)
            .context("failed to set TLS configuration")?
            .connect()
            .await
            .context("failed to connect gRPC channel")
            .map(Client::from)
    }
}

pub(crate) struct Client<S: State> {
    channel: Channel,
    _state: PhantomData<S>,
}

impl From<Channel> for Client<New> {
    fn from(channel: Channel) -> Self {
        Self {
            channel,
            _state: PhantomData,
        }
    }
}

impl Client<New> {
    const CLIENT_ID: &str = "bgpfu-junos-agent-0.0.0";
    const GROUP_ID: &str = "";

    pub(crate) async fn authenticate(
        self,
        username: String,
        password: String,
    ) -> anyhow::Result<Client<Authenticated>> {
        log::debug!("attempting to authenticate to JET server");
        let mut channel = self.channel;
        let req = LoginRequest {
            client_id: Self::CLIENT_ID.to_string(),
            group_id: Self::GROUP_ID.to_string(),
            username,
            password,
        };
        let resp = AuthenticationClient::new(&mut channel)
            .login(req)
            .await
            .context("JET login request failed")?
            .into_inner();
        if let Some(status) = resp.status {
            match status.code() {
                StatusCode::Success => {
                    log::info!("authentication successful");
                    Ok(Client {
                        channel,
                        _state: PhantomData,
                    })
                }
                StatusCode::Failure => bail!("authentication failed: {}", status.message),
            }
        } else {
            bail!("no status in login response message: {:?}", resp);
        }
    }
}

impl Client<Authenticated> {
    pub(crate) async fn op_command(&mut self, command: String) -> anyhow::Result<String> {
        log::debug!("attempting to run operational cmd");
        let req = OpCommandGetRequest {
            out_format: OpCommandOutputFormat::OpCommandOutputCli.into(),
            command: Some(Command::CliCommand(command)),
        };
        let mut resp_stream = ManagementClient::new(&mut self.channel)
            .op_command_get(req)
            .await
            .context("JET operational command request failed")?
            .into_inner();
        let mut data = String::new();
        while let Some(resp) = resp_stream.message().await? {
            if let Some(status) = resp.status {
                match status.code() {
                    StatusCode::Success => data.push_str(&resp.data),
                    StatusCode::Failure => bail!("op command failed: {}", status.message),
                };
            } else {
                bail!("no status in response message: {:?}", resp);
            };
        }
        Ok(data)
    }
}
