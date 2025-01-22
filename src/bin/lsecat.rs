//! List the devices visibile on the EtherCAT network

use std::{sync::Arc, time::Duration};

use argh::FromArgs;
use ethercrab::{
    error::Error,
    std::{ethercat_now, tx_rx_task},
    MainDevice, MainDeviceConfig, PduStorage, SubDeviceIdentity, Timeouts,
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
    #[argh(switch, short = 'd')]
    /// show the manufacturer-provided description.  If it contains
    /// whitespace, it will be displayed as a double-quoted string.
    description: bool,
    #[argh(switch, short = 'i')]
    /// show vendor, product, revision, and serial numbers
    identity: bool,
    #[argh(switch, short = 'l')]
    /// show all available information for each device
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

    let group = group.into_op(&maindevice).await.expect("PRE-OP -> OP");

    for subdevice in group.iter(&maindevice) {
        print!(
            "{:#06x} {}",
            subdevice.configured_address(),
            subdevice.name()
        );
        if cli.description || cli.long {
            if let Some(description) = subdevice.description().await? {
                print!(" description:{}", escape(&description));
            } else {
                print!(" description:\"\"");
            }
        }
        if cli.identity || cli.long {
            print!(" {}", fmt_identity(subdevice.identity()));
        }
        if cli.long {
            let io = subdevice.io_raw();
            print!(
                " alias:{:#06x} delay:{}ns in:{}B out:{}B",
                subdevice.alias_address(),
                subdevice.propagation_delay(),
                io.inputs().len(),
                io.inputs().len()
            );
        }
        println!();
    }

    let group = group
        .into_safe_op(&maindevice)
        .await
        .expect("OP -> SAFE-OP");

    let group = group
        .into_pre_op(&maindevice)
        .await
        .expect("SAFE-OP -> PRE-OP");

    let _group = group.into_init(&maindevice).await.expect("PRE-OP -> INIT");

    Ok(())
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
