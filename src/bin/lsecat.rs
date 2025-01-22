//! List the devices visibile on the EtherCAT network

use std::{sync::Arc, time::Duration};

use argh::FromArgs;
use ethercrab::{
    error::Error,
    std::{ethercat_now, tx_rx_task},
    subdevice_group::{Init, Op},
    MainDevice, MainDeviceConfig, PduStorage, SubDeviceGroup, SubDeviceIdentity, Timeouts,
};

/// Maximum number of SubDevices that can be stored. This must be a power of 2 greater than 1.
const MAX_SUBDEVICES: usize = 16;
/// Maximum PDU data payload size - set this to the max PDI size or higher.
const MAX_PDU_DATA: usize = PduStorage::element_size(1100);
/// Maximum number of EtherCAT frames that can be in flight at any one time.
const MAX_FRAMES: usize = 16;
/// Maximum total PDI length.
const PDI_LEN: usize = 2048;

static PDU_STORAGE: PduStorage<MAX_FRAMES, MAX_PDU_DATA> = PduStorage::new();

#[derive(FromArgs)]
/// List all the devices on the connected EtherCAT network.
///
/// Without any options, this will show the EtherCAT address and name
/// of each devices on a line.
struct Cli {
    #[argh(positional)]
    /// the network interface the EtherCAT bus is connected to
    interface: String,
    #[argh(switch)]
    /// show all available metadata for each device
    meta: bool,
    #[argh(switch)]
    /// show information about the PDO lengths; requires that the
    /// network can enter OP
    pdo: bool,
    #[argh(switch, short = 'l')]
    /// show all available data about the device; requires that the
    /// network can enter OP
    long: bool,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli: Cli = argh::from_env();

    let (tx, rx, pdu_loop) = PDU_STORAGE.try_split().expect("can only split once");

    let maindevice = Arc::new(MainDevice::new(
        pdu_loop,
        Timeouts {
            wait_loop_delay: Duration::from_millis(2),
            mailbox_response: Duration::from_millis(1000),
            ..Default::default()
        },
        MainDeviceConfig::default(),
    ));

    match tx_rx_task(&cli.interface, tx, rx) {
        Ok(task) => tokio::spawn(task),
        Err(err) => {
            println!("{err}");
            std::process::exit(1);
        }
    };

    let Ok(group) = maindevice
        .init_single_group::<MAX_SUBDEVICES, PDI_LEN>(ethercat_now)
        .await
    else {
        println!("failed to init; EtherCAT bus could be on a different interface, disconnected, or timing out");
        std::process::exit(1);
    };

    let mut subdevice_datas: Vec<SubdeviceData> = group
        .iter(&maindevice)
        .map(|subdevice| SubdeviceData::new(subdevice.name(), subdevice.configured_address()))
        .collect();

    if cli.meta || cli.long {
        for (i, subdevice) in group.iter(&maindevice).enumerate() {
            subdevice_datas[i].description = Some(
                subdevice
                    .description()
                    .await?
                    .unwrap_or_default()
                    .to_string(),
            );
            subdevice_datas[i].identity = Some(subdevice.identity());
            subdevice_datas[i].alias_address = Some(subdevice.alias_address());
            subdevice_datas[i].propagation_delay = Some(subdevice.propagation_delay());
        }
    }

    if !(cli.pdo || cli.long) {
        for datum in subdevice_datas {
            println!("{datum}");
        }
        group.into_init(&maindevice).await?;
        return Ok(());
    }

    let group = group.into_op(&maindevice).await?;

    for (i, subdevice) in group.iter(&maindevice).enumerate() {
        let io = subdevice.io_raw();
        subdevice_datas[i].input_len = Some(io.inputs().len());
        subdevice_datas[i].output_len = Some(io.outputs().len());
    }

    for datum in subdevice_datas {
        println!("{datum}");
    }

    let _group = close_ethercat(group, maindevice).await?;

    Ok(())
}

async fn close_ethercat(
    group: SubDeviceGroup<{ MAX_SUBDEVICES }, { PDI_LEN }, Op>,
    maindevice: Arc<MainDevice<'_>>,
) -> Result<SubDeviceGroup<{ MAX_SUBDEVICES }, { PDI_LEN }, Init>, Error> {
    let group = group.into_safe_op(&maindevice).await?;

    let group = group.into_pre_op(&maindevice).await?;

    group.into_init(&maindevice).await
}

struct SubdeviceData {
    name: String,
    address: u16,
    description: Option<String>,
    identity: Option<SubDeviceIdentity>,
    alias_address: Option<u16>,
    propagation_delay: Option<u32>,
    input_len: Option<usize>,
    output_len: Option<usize>,
}

impl std::fmt::Display for SubdeviceData {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:#06x} {}", self.address, self.name)?;
        if let Some(description) = &self.description {
            write!(f, " description:{}", escape(description))?;
        }
        if let Some(identity) = self.identity {
            write!(f, " {}", fmt_identity(identity))?;
        }
        if let Some(alias_address) = self.alias_address {
            write!(f, " alias:{:#06x}", alias_address)?;
        }
        if let Some(delay) = self.propagation_delay {
            write!(f, " delay:{}ns", delay)?;
        }
        if let Some(i) = self.input_len {
            write!(f, " in:{}B", i)?;
        }
        if let Some(o) = self.output_len {
            write!(f, " out:{}B", o)?;
        }
        Ok(())
    }
}

impl SubdeviceData {
    fn new(name: &str, address: u16) -> Self {
        Self {
            name: name.into(),
            address,
            description: None,
            identity: None,
            alias_address: None,
            propagation_delay: None,
            input_len: None,
            output_len: None,
        }
    }
}

fn escape(s: &str) -> String {
    if !s.contains([' ', '\t', '\n', '\r']) {
        s.into()
    } else {
        let escape_idxs =
            s.chars().enumerate().filter_map(
                |(i, c)| {
                    if c == '"' || c == '\\' {
                        Some(i)
                    } else {
                        None
                    }
                },
            );
        let mut s: String = s.to_string();
        for (i, idx) in escape_idxs.enumerate() {
            s.insert(idx + i, '\\');
        }
        format!("\"{}\"", s)
    }
}

fn fmt_identity(identity: SubDeviceIdentity) -> String {
    format!(
        "vendor:{:#010x} product:{:#010x} rev:{} serial:{}",
        identity.vendor_id, identity.product_id, identity.revision, identity.serial
    )
}
