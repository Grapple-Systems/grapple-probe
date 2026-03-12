// Copyright (c) 2026 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use grapple_probe_lib::{GrappleProbe, PowerControl};

#[derive(Debug, clap::Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// serial number of a grapple probe to use for commands
    #[arg(short, long)]
    serial: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, clap::Subcommand)]
enum Commands {
    /// List all connected Grapple Probes
    List,

    /// Show the status of selected Grapple Probes
    Status,

    /// Display and modify power control configurations
    PowerControl(PowerControlArgs),

    /// Read raw configuration fields
    ReadConfig(ReadConfigArgs),

    /// Write raw configuration fields
    WriteConfig(WriteConfigArgs),
}

#[derive(Debug, clap::Args)]
struct ReadConfigArgs {
    /// Id of config field to read
    id: u8,
}

fn parse_hex_str(val: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(val)
}

#[derive(Debug, clap::Args)]
struct WriteConfigArgs {
    /// Id of config field to write
    id: u8,

    // Data to write to the config field as a hexadecimal string
    #[arg(value_parser=parse_hex_str)]
    data: Vec<u8>,
}

#[derive(Debug, clap::Args)]
struct PowerControlArgs {
    /// Set the power control to default
    #[arg(short, long)]
    default: bool,

    /// Should signal voltage be gated by the ground detect signal
    #[arg(long)]
    signal_gnddet: Option<bool>,

    /// Should signal voltage be gated by the detection of a target signal
    #[arg(long)]
    signal_target: Option<bool>,

    /// Should signal voltage follow the measured target voltage
    #[arg(long)]
    follow_target: Option<bool>,

    /// Signal voltage when not following the target voltage in millivolts
    #[arg(long)]
    signal: Option<u16>,

    /// Should target voltage be an output
    #[arg(long)]
    target_output: Option<bool>,

    /// Should target voltage be gated by the ground detect signals
    #[arg(long)]
    target_gnddet: Option<bool>,

    /// Target voltage when acting as an output in mv
    #[arg(long)]
    target: Option<u16>,

    /// Should the 5VKey power be enabled
    #[arg(long)]
    key5v: Option<bool>,
}

fn list(probes: impl Iterator<Item = GrappleProbe>) {
    for probe in probes {
        println!("graple probe: {}", probe.get_serial_number());
    }
}

fn status(probes: impl Iterator<Item = GrappleProbe>) {
    for probe in probes {
        println!("graple probe: {}", probe.get_serial_number());
        if let Ok(mut dap) = probe.open_cmsis_dap() {
            if let Ok(fw_version) = dap.get_firmware_version() {
                println!("\tfw version: {}", fw_version);
            } else {
                println!("\tfailed to get firmware version");
            }
            if let Ok(status) = dap.get_status() {
                println!("\tgnd detected: {}", status.gnddet);
                println!("\ttvcc: {} mv", status.target_mv);
                println!("\tsignal: {} mv", status.signal_mv);
            }
            else {
                println!("\tfailed to get status");
            }
        } else {
            println!("\tfailed to open probe");
        }
    }
}

fn power_control(probes: impl Iterator<Item = GrappleProbe>, args: PowerControlArgs) {
    let probes: Vec<GrappleProbe> = probes.collect();
    if let Some(probe) = (probes.len() == 1).then(|| probes.first().unwrap()) {
        println!("graple probe: {}", probe.get_serial_number());
        if let Ok(mut dap) = probe.open_cmsis_dap() {
            if let Ok(()) = dap.modify_power_config(|cfg| {
                    let mut changed = false;
                    if args.default { cfg.clone_from(&PowerControl::default()); changed = true; }
                    if let Some(follow_target) = args.follow_target { cfg.signal_follows_tvcc = follow_target; changed = true; }
                    if let Some(signal_gnddet) = args.signal_gnddet { cfg.signal_gated_by_gnddet = signal_gnddet; changed = true; }
                    if let Some(signal_target) = args.signal_target { cfg.signal_gated_by_tvcc = signal_target; changed = true; }
                    if let Some(signal) = args.signal { cfg.signal_default_mv = signal; changed = true; }

                    if let Some(key5v) = args.key5v { cfg.key5v_enable = key5v; changed = true; }
                    if let Some(target_output) = args.target_output { cfg.tvcc_output = target_output; changed = true; }
                    if let Some(target_gnddet) = args.target_gnddet { cfg.tvcc_gated_by_gnddet = target_gnddet; changed = true; }
                    if let Some(target) = args.target { cfg.tvcc_output_mv = target; changed = true; }
                    changed
                })
            {
                if let Ok(config) = dap.get_power_config() {
                    println!("\t5VKey enabled: {}", config.key5v_enable);
                    let gnddet = if config.signal_gated_by_gnddet { "[gnddet]" } else { "gnddet" };
                    let tvcc = if config.signal_gated_by_tvcc { "[tvcc]" } else { "tvcc" };
                    let follow = if config.signal_follows_tvcc { "[follow]" } else { "follow" };
                    println!("\tsignal supply ({} mv): {} {} {}", config.signal_default_mv, gnddet, tvcc, follow);
                    let dir = if config.tvcc_output { "[out]" } else { "[in]" };
                    let gnddet = if config.tvcc_gated_by_gnddet { "[gnddet]" } else { "gnddet" };
                    println!("\ttarget supply ({} mv): {} {}", config.tvcc_output_mv, dir, gnddet);
                }
                else {
                    println!("\tfailed to get the power config");
                }
            } else {
                println!("\tfailed to set the power config");
            }
        }
    } else {
        println!("please select only one probe using the --serial argument")
    }
}

fn read_field(mut probes: impl Iterator<Item = GrappleProbe>, args: ReadConfigArgs) {
    if let Some(probe) = probes.next() {
        println!("graple probe: {}", probe.get_serial_number());
        if let Ok(mut dap) = probe.open_cmsis_dap() {
            if let Ok(field_data) = dap.read_field(args.id) {
                println!("\t{}: {}", args.id, hex::encode(field_data));
            }
            else {
                println!("\tfailed to read field");
            }
        } else {
            println!("\tfailed to open probe");
        }
    }
}

fn write_field(probes: impl Iterator<Item = GrappleProbe>, args: WriteConfigArgs) {
    let probes: Vec<GrappleProbe> = probes.collect();
    if let Some(probe) = (probes.len() == 1).then(|| probes.first().unwrap()) {
        println!("graple probe: {}", probe.get_serial_number());
        if let Ok(mut dap) = probe.open_cmsis_dap() {
            if dap.write_field(args.id, &args.data).is_ok() {
                println!("\tsuccess");
            }
            else {
                println!("\tfailed to write field");
            }
        } else {
            println!("\tfailed to open probe");
        }
    } else {
        println!("please select only one probe using the --serial argument")
    }
}

fn main() {
    use clap::Parser;

    let args = Args::parse();

    let probes = GrappleProbe::all().filter(|probe| {
        args.serial.as_ref().is_none_or(|serial| probe.get_serial_number() == serial.as_str())
    });

    match args.command {
        Commands::List => list(probes),
        Commands::Status => status(probes),
        Commands::PowerControl(args) => power_control(probes, args),
        Commands::ReadConfig(args) => read_field(probes, args),
        Commands::WriteConfig(args) => write_field(probes, args),
    }
}
