use sparkles::config::SparklesConfig;

#[derive(clap::Parser, Debug, Default)]
pub struct Args {
    /// Enable multicast discovery (default: false)
    /// If disabled, app will not wait for Sparkles client to connect and write trace data to file instead
    #[clap(long, short, default_value_t = false)]
    discovery: bool,
    /// Use specific UDP port for Sparkles sender
    /// Usually not needed, because discovery will announce local port
    #[clap(long, short='p', value_parser = clap::value_parser!(u16))]
    udp_port: Option<u16>,
}

impl Args {
    pub fn sparkles_cfg(&self) -> SparklesConfig {
        let mut cfg = SparklesConfig::default();
        if self.discovery {
            cfg = cfg.with_udp_multicast_default();
        }
        if let Some(port) = self.udp_port {
            cfg = cfg.with_udp_sender(port);
        }
        cfg
    }
}
