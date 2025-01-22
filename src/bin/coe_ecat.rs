use std::str::FromStr;
use std::sync::{mpsc, Arc};

use ethercrab::{
    // error::Error,
    std::{ethercat_now, tx_rx_task},
    MainDevice,
    MainDeviceConfig,
    PduStorage,
    Timeouts,
};
use tokio::time::{self, Duration, MissedTickBehavior};

use ecat_utils::explorer_parser::Command;

/// Maximum number of slaves that can be stored. This must be a power of 2 greater than 1.
const MAX_SLAVES: usize = 16;
/// Maximum PDU data payload size - set this to the max PDI size or higher.
const MAX_PDU_DATA: usize = PduStorage::element_size(1100);
/// Maximum number of EtherCAT frames that can be in flight at any one time.
const MAX_FRAMES: usize = 16;
/// Maximum total PDI length.
const PDI_LEN: usize = 64;

static PDU_STORAGE: PduStorage<MAX_FRAMES, MAX_PDU_DATA> = PduStorage::new();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (tx, rx, pdu_loop) = PDU_STORAGE.try_split().expect("can only split once");

    let interface: String;
    loop {
        if let Ok(iface) = input("network interface: ") {
            interface = iface;
            break;
        }
    }

    tokio::spawn(tx_rx_task(&interface, tx, rx)?);

    let main_device = Arc::new(MainDevice::new(
        pdu_loop,
        Timeouts {
            wait_loop_delay: Duration::from_millis(2),
            mailbox_response: Duration::from_millis(1000),
            ..Default::default()
        },
        MainDeviceConfig::default(),
    ));

    let group = main_device
        .init_single_group::<MAX_SLAVES, PDI_LEN>(ethercat_now)
        .await
        .expect("Init");

    let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&shutdown))
        .expect("Register hook");

    loop {
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            println!("Shutting down...");
            break;
        }

        let Ok(command) = interactive_tty() else {
            println!("sorry, I didn't understand that");
            continue;
        };
        match command {
            Command::Read(read) => {
                let Some(subdevice) = group
                    .iter(&main_device)
                    .find(|subdevice| subdevice.name() == read.name())
                else {
                    println!("no ethercat devices connected");
                    continue;
                };
                subdevice.sdo_read
            }
            Command::Write(write) => todo!(),
        }
    }

    println!("press p to print the latest PDO contents");

    let group = group.into_op(&main_device).await.expect("PRE-OP -> OP");

    let mut tick_interval = time::interval(Duration::from_millis(10));
    tick_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        // graceful shutdown on ^C
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            println!("Shutting down...");
            break;
        }
        group.tx_rx(&main_device).await.expect("TX/RX");

        if let Some(el3062) = group
            .iter(&main_device)
            .find(|slave| slave.name() == "EL3062")
        {
            let _pdos = el3062.io_raw();
            // if let Ok(channel1) = El3062Reading::unpack_from_slice(&i[..4]) {
            // measurement_signal = Some(channel1.value as f64 / u16::MAX as f64);
            // }
        }

        tick_interval.tick().await;
    }

    let group = group
        .into_safe_op(&main_device)
        .await
        .expect("OP -> SAFE-OP");
    let group = group
        .into_pre_op(&main_device)
        .await
        .expect("SAFE-OP -> PRE-OP");
    let _group = group.into_init(&main_device).await.expect("PRE-OP -> INIT");

    Ok(())
}

fn input(prompt: &str) -> std::io::Result<String> {
    print!("{prompt}");
    let mut out = String::new();
    std::io::stdin().read_line(&mut out)?;
    Ok(out)
}

fn interactive_tty() -> Result<Command, ()> {
    let mut command_string = String::new();
    std::io::stdin().read_line(&mut command_string).unwrap();
    Command::from_str(&command_string)
}
