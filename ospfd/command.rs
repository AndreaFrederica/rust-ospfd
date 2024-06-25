use std::{collections::HashMap, process::exit, sync::OnceLock};

use lazy_static::lazy_static;
use ospf_packet::lsa;
use trie_rs::{Trie, TrieBuilder};

use crate::{
    area::Area, database::ProtocolDB, guard, interface::InterfaceEvent, log, log_success, must,
};

struct CommandSet {
    /// supported commands
    trie: Trie<u8>,
    /// command descriptions
    desc: HashMap<&'static str, &'static str>,
    /// handler for the command
    handlers: HashMap<&'static str, Box<dyn Fn() -> &'static CommandSet + Sync>>,
    /// handler for running this command
    handle_enter: Option<Box<dyn Fn() + Sync>>,
    /// handler for the command with arbitrary argument
    arbitrary: Option<Box<dyn Fn(&str) -> &'static CommandSet + Sync>>,
}

impl CommandSet {
    fn new(
        desc: HashMap<&'static str, &'static str>,
        handlers: HashMap<&'static str, Box<dyn Fn() -> &'static CommandSet + Sync>>,
        handle_enter: Option<Box<dyn Fn() + Sync>>,
        arbitrary: Option<Box<dyn Fn(&str) -> &'static CommandSet + Sync>>,
    ) -> Self {
        let mut builder = TrieBuilder::new();
        desc.keys()
            .filter(|s| !s.starts_with("<"))
            .for_each(|&s| builder.push(s));
        CommandSet {
            trie: builder.build(),
            desc,
            handlers,
            handle_enter,
            arbitrary,
        }
    }
}

macro_rules! command {
    (
        $(enter: ($ve:literal) => $fe:expr;)? // 直接敲下回车要执行的
        $(arg: $ka:literal ($va:literal) => $fa:expr;)? // 有任意参数的
        $($k:literal ($v:literal) => $f:expr;)* // 固定匹配指令的
    ) => { #[allow(unused_mut, unused_assignments)] {
        let mut desc = HashMap::<&str, &str>::new();
        let mut handlers = HashMap::<&str, Box<dyn Fn() -> &'static CommandSet + Sync>>::new();
        let mut handle_enter = Option::<Box<dyn Fn() + Sync>>::None;
        let mut arbitrary = Option::<Box<dyn Fn(&str) -> &'static CommandSet + Sync>>::None;
        $(
            desc.insert("<enter>", $ve);
            handle_enter = Some(Box::new($fe));
        )?
        $(
            desc.insert($ka, $va);
            arbitrary = Some(Box::new($fa));
        )?
        $(
            desc.insert($k, $v);
            handlers.insert($k, Box::new($f));
        )*
        CommandSet::new(desc, handlers, handle_enter, arbitrary)
    }};
}

lazy_static! {
    static ref ROOT: CommandSet = command! {
        enter: ("run nothing") => || {};
        "display"("display something...") => parse_display;
        "interface"("interface setting...") => parse_interface;
        "exit"("exit ospfd") => parse_exit;
    };
}

pub static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();

macro_rules! error {
    ($raw:expr, $cur:expr, $msg:expr) => {{
        let idx = unsafe { $cur.as_ptr().offset_from($raw.as_ptr()) } as usize;
        crate::log_error!("{}\n{}^ {}", $raw, " ".repeat(idx), $msg);
        return;
    }};
}

fn display_help(desc: &HashMap<&str, &str>) {
    let max_key_len = desc.keys().map(|s| s.len()).max().unwrap();
    let mut vec: Vec<_> = desc.iter().collect();
    vec.sort_by_key(|(&k, _)| k);
    for (k, v) in vec {
        crate::log!("  {:<width$} - {}", k, v, width = max_key_len);
    }
}

pub fn parse_cmd(raw: String) {
    let raw = raw.trim();
    let mut list = raw.split_ascii_whitespace();
    let mut set: &CommandSet = &ROOT;
    while let Some(cmd) = list.next() {
        if cmd == "?" {
            display_help(&set.desc);
            return;
        }
        let (cmd, q) = if cmd.ends_with("?") {
            (&cmd[..cmd.len() - 1], true)
        } else {
            (cmd, false)
        };
        let matches: Vec<String> = set.trie.predictive_search(cmd).collect();
        if matches.is_empty() {
            if let Some(ref hd) = set.arbitrary {
                set = hd(cmd);
            } else {
                error!(raw, cmd, "bad command");
            }
        } else if q {
            display_help(
                &matches
                    .into_iter()
                    .map(|s| set.desc.get_key_value(s.as_str()).unwrap())
                    .map(|(&k, &v)| (k, v))
                    .collect(),
            );
            return;
        } else if matches.len() > 1 {
            error!(raw, cmd, "ambiguous command");
        } else {
            set = set.handlers.get(matches[0].as_str()).unwrap()();
        }
    }
    if let Some(ref hd) = set.handle_enter {
        hd();
    } else {
        error!(raw, raw[raw.len() - 1..], "bad command");
    }
}

macro_rules! block_on {
    ($e:expr) => {
        RUNTIME.get().unwrap().block_on($e)
    };
}

fn parse_display() -> &'static CommandSet {
    lazy_static! {
        static ref DISPLAY: CommandSet = command! {
            "routing"("display routing table") => parse_display_routing;
            "peer"("display ospf neighbors") => parse_display_peer;
            "lsdb"("display ospf link state database") => parse_display_lsdb;
        };
    };
    &DISPLAY
}

fn parse_display_routing() -> &'static CommandSet {
    lazy_static! {
        static ref DISPLAY: CommandSet = command! {
            enter: ("display routing table") => || log!("{}", block_on!(ProtocolDB::get()).routing_table);
            "system" ("display system routing table") => parse_display_routing_system;
        };
    };
    &DISPLAY
}

fn parse_display_routing_system() -> &'static CommandSet {
    lazy_static! {
        static ref DISPLAY: CommandSet = command! {
            enter: ("display system routing table") => || { let _ = std::process::Command::new("route").status(); };
        };
    };
    &DISPLAY
}

fn parse_display_peer() -> &'static CommandSet {
    lazy_static! {
        static ref DISPLAY: CommandSet = command! {
            enter: ("display ospf neighbors") => || {
                log!("\tOSPF with Router ID: {}", ProtocolDB::get_router_id());
                log!("\t\tNeighbors");
                ProtocolDB::get_interfaces_impl().iter().for_each(|iface| {
                    log!("Area {} interface {}({})'s neighbors", iface.area_id, iface.ip_addr, iface.interface_name);
                    iface.neighbors.values().for_each(|n| log!("{}", n));
                });
            };
        };
    };
    &DISPLAY
}

fn parse_display_lsdb() -> &'static CommandSet {
    lazy_static! {
        static ref DISPLAY: CommandSet = command! {
            enter: ("display ospf link state database") => || {
                log!("\tOSPF with Router ID: {}", ProtocolDB::get_router_id());
                log!("\t\tLink State Database");
                block_on!(ProtocolDB::get()).areas.values().for_each(|area| {
                    let mut lsa = area.get_all_area_lsa();
                    must!(lsa.len() > 0);
                    log!("\t\t\tArea: {}", area.area_id);
                    log!("Type      LinkState ID    AdvRouter       Age   Len   Sequence");
                    lsa.sort_by_key(|lsa| lsa.ls_type);
                    lsa.into_iter().for_each(|lsa| {
                        log!("{:<9} {:<15} {:<15} {:<5} {:<5} {:<10X}",
                            lsa::types::to_string(lsa.ls_type), lsa.link_state_id,
                            lsa.advertising_router, lsa.ls_age, lsa.length, lsa.ls_sequence_number);
                    });
                });
                let mut lsa = block_on!(Area::get_all_external_lsa());
                must!(lsa.len() > 0);
                log!("\t\tAS External Database");
                log!("Type      LinkState ID    AdvRouter       Age   Len   Sequence");
                lsa.sort_by_key(|(lsa, _)| lsa.ls_type);
                lsa.into_iter().for_each(|(lsa, _)| {
                    log!("{:<9} {:<15} {:<15} {:<5} {:<5} {:<10X}",
                        lsa::types::to_string(lsa.ls_type), lsa.link_state_id,
                        lsa.advertising_router, lsa.ls_age, lsa.length, lsa.ls_sequence_number);
                });
            };
        };
    };
    &DISPLAY
}

fn parse_interface() -> &'static CommandSet {
    lazy_static! {
        static ref IFACE: CommandSet = command! {
            arg: "<iface_name>"("interface setting...") => parse_interface_name;
        };
    };
    &IFACE
}

fn parse_interface_name(name: &str) -> &'static CommandSet {
    static mut IFACE: Option<CommandSet> = None;
    static mut NAME: String = String::new();
    unsafe {
        NAME = name.to_string();
        IFACE = Some(command! {
            "area_id"("interface area setting") => || parse_interface_area(NAME.clone());
        });
        IFACE.as_ref().unwrap()
    }
}

fn parse_interface_area(name: String) -> &'static CommandSet {
    static mut IFACE: Option<CommandSet> = None;
    static mut NAME: String = String::new();
    unsafe {
        NAME = name;
        IFACE = Some(command! {
            arg: "<area_id>"("interface area setting") => |arg| parse_interface_area_id(NAME.clone(), arg);
        });
        &IFACE.as_ref().unwrap()
    }
}

fn parse_interface_area_id(name: String, arg: &str) -> &'static CommandSet {
    static mut IFACE: Option<CommandSet> = None;
    let arg = arg.to_string();
    unsafe {
        IFACE = Some(command! {
            enter: ("changing interface area id") => move || {
                guard!(Some(mut iface) = ProtocolDB::get_interface_by_name(name.as_str()); error: "bad interface_name: {name}");
                guard!(Ok(id) = arg.parse(); error: "bad area_id: {arg}");
                log_success!("Interface {}'s area id is changed to {}", iface.interface_name, arg);
                block_on!(iface.interface_down());
                block_on!(ProtocolDB::get()).areas.insert(id, Area::new(id));
                iface.area_id = id;
                block_on!(iface.interface_up());
            };
        });
        IFACE.as_ref().unwrap()
    }
}

fn parse_exit() -> &'static CommandSet {
    lazy_static! {
        static ref EXIT: CommandSet = command! {
            enter: ("exit ospfd") => || { block_on!(ProtocolDB::get()).routing_table.delete_all_routing(); exit(0); };
        };
    };
    &EXIT
}
