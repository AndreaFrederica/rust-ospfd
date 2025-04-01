use std::{
    collections::{HashMap, VecDeque},
    io::{stdout, Write},
    process::exit,
    sync::{Mutex, OnceLock},
};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers}, // 移除了 KeyModifiers
    execute,
    terminal::{self, Clear, ClearType},
};
use lazy_static::lazy_static;
use trie_rs::{Trie, TrieBuilder};

use crate::{
    area::Area, database::ProtocolDB, guard, interface::InterfaceEvent, log, log_success, must,
};

use tokio::signal;

/// 最大保存 50 条历史命令
const MAX_HISTORY: usize = 50;

/// 全局历史命令记录
lazy_static! {
    static ref COMMAND_HISTORY: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
}

/// 命令树结构
struct CommandSet {
    /// 支持的命令（Trie 自动匹配）
    trie: Trie<u8>,
    /// 命令描述
    desc: HashMap<&'static str, &'static str>,
    /// 命令对应的处理函数
    handlers: HashMap<&'static str, Box<dyn Fn() -> &'static CommandSet + Sync>>,
    /// 直接敲回车执行的处理函数
    handle_enter: Option<Box<dyn Fn() + Sync>>,
    /// 带任意参数的命令处理函数
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
        // 过滤掉以 "<" 开头的特殊描述
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

/// 宏定义方便构造 CommandSet
macro_rules! command {
    (
        $(enter: ($ve:literal) => $fe:expr;)?
        $(arg: $ka:literal ($va:literal) => $fa:expr;)?
        $($k:literal ($v:literal) => $f:expr;)*
    ) => {{
        #[allow(unused_mut, unused_assignments)]
        let mut desc = HashMap::<&str, &str>::new();
        let mut handlers =
            HashMap::<&str, Box<dyn Fn() -> &'static CommandSet + Sync>>::new();
        let mut handle_enter = Option::<Box<dyn Fn() + Sync>>::None;
        let mut arbitrary =
            Option::<Box<dyn Fn(&str) -> &'static CommandSet + Sync>>::None;
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

/// 用于异步操作的全局运行时句柄
pub static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();

/// 处理命令错误时调用的宏
macro_rules! error {
    ($raw:expr, $cur:expr, $msg:expr) => {{
        let idx = unsafe { $cur.as_ptr().offset_from($raw.as_ptr()) } as usize;
        crate::log_error!("{}\r\n{}^ {}", $raw, " ".repeat(idx), $msg);
        return;
    }};
}

/// 显示当前命令集合的帮助信息
fn display_help(desc: &HashMap<&str, &str>) {
    let max_key_len = desc.keys().map(|s| s.len()).max().unwrap();
    let mut vec: Vec<_> = desc.iter().collect();
    vec.sort_by_key(|(&k, _)| k);
    for (k, v) in vec {
        crate::log!("  {:<width$} - {}", k, v, width = max_key_len);
    }
}

/// 根据用户输入的命令字符串进行解析并执行对应命令
pub fn parse_cmd(raw: String) {
    if !raw.ends_with('\n') {
        log!();
    }
    let raw = raw.trim().to_lowercase();
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
        error!(raw, &raw[raw.len() - 1..], "bad command");
    }
}

/// 用于在命令中执行异步操作
macro_rules! block_on {
    ($e:expr) => {
        RUNTIME.get().unwrap().block_on($e)
    };
}

/// display 相关命令
fn parse_display() -> &'static CommandSet {
    lazy_static! {
        static ref DISPLAY: CommandSet = command! {
            "routing"("display routing table") => parse_display_routing;
            "peer"("display ospf neighbors") => parse_display_peer;
            "lsdb"("display ospf link state database") => parse_display_lsdb;
        };
    }
    &DISPLAY
}

fn parse_display_routing() -> &'static CommandSet {
    lazy_static! {
        static ref DISPLAY: CommandSet = command! {
            enter: ("display routing table") => || log!("{}", block_on!(ProtocolDB::get()).routing_table);
            "system" ("display system routing table") => parse_display_routing_system;
        };
    }
    &DISPLAY
}

fn parse_display_routing_system() -> &'static CommandSet {
    lazy_static! {
        static ref DISPLAY: CommandSet = command! {
            enter: ("display system routing table") => || { let _ = std::process::Command::new("route").status(); };
        };
    }
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
    }
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
                    lsa.into_iter().for_each(|lsa| log!("{}", lsa));
                });
                let mut lsa = block_on!(Area::get_all_external_lsa());
                must!(lsa.len() > 0);
                log!("\t\tAS External Database");
                log!("Type      LinkState ID    AdvRouter       Age   Len   Sequence");
                lsa.sort_by_key(|(lsa, _)| lsa.ls_type);
                lsa.into_iter().for_each(|(lsa, _)| log!("{}", lsa));
            };
        };
    }
    &DISPLAY
}

/// interface 相关命令
fn parse_interface() -> &'static CommandSet {
    lazy_static! {
        static ref IFACE: CommandSet = command! {
            arg: "<iface_name>"("interface setting...") => parse_interface_name;
        };
    }
    &IFACE
}

fn parse_interface_name(name: &str) -> &'static CommandSet {
    static mut IFACE: Option<CommandSet> = None;
    static mut NAME: String = String::new();
    unsafe {
        NAME = name.to_string();
        IFACE = Some(command! {
            "area_id"("interface area setting") => || parse_interface_area(NAME.clone());
            "cost"("interface cost setting") => || parse_interface_cost(NAME.clone());
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

fn parse_interface_cost(name: String) -> &'static CommandSet {
    static mut IFACE: Option<CommandSet> = None;
    static mut NAME: String = String::new();
    unsafe {
        NAME = name;
        IFACE = Some(command! {
            arg: "<cost>"("interface cost setting") => |arg| parse_interface_cost_set(NAME.clone(), arg);
        });
        &IFACE.as_ref().unwrap()
    }
}

fn parse_interface_cost_set(name: String, arg: &str) -> &'static CommandSet {
    static mut IFACE: Option<CommandSet> = None;
    let arg = arg.to_string();
    unsafe {
        IFACE = Some(command! {
            enter: ("changing interface cost") => move || {
                guard!(Some(mut iface) = ProtocolDB::get_interface_by_name(name.as_str()); error: "bad interface_name: {name}");
                guard!(Ok(cost) = arg.parse(); error: "bad cost: {arg}");
                iface.cost = cost;
            };
        });
        IFACE.as_ref().unwrap()
    }
}

// fn parse_exit() -> &'static CommandSet {
//     lazy_static! {
//         static ref EXIT: CommandSet = command! {
//             enter: ("exit ospfd") => || { block_on!(ProtocolDB::get()).routing_table.delete_all_routing(); exit(0); };
//         };
//     }
//     &EXIT
// }
fn parse_exit() -> &'static CommandSet {
    lazy_static! {
        static ref EXIT: CommandSet = command! {
            enter: ("exit ospfd") => || {
                // 异步清理路由表，不阻塞当前线程
                tokio::spawn(async {
                    // 尽量忽略错误或记录日志
                    let _ = ProtocolDB::get().await.routing_table.delete_all_routing();
                });
                // 直接退出程序
                std::process::exit(0);
            };
        };
    }
    &EXIT
}

struct LineEditor {
    buffer: String,
    history_index: Option<usize>,
    cursor_position: usize, // 新增光标位置跟踪
}

impl LineEditor {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            history_index: None,
            cursor_position: 0, // 初始化光标位置
        }
    }

    fn read_line(&mut self) -> String {
        let mut stdout = stdout();
        self.buffer.clear();
        self.cursor_position = 0; // 重置光标位置
        self.history_index = None;

        execute!(
            stdout,
            cursor::MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            cursor::Show // 显示光标
        )
        .unwrap();
        write!(stdout, "ospfd> ").unwrap();
        stdout.flush().unwrap();

        loop {
            if let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read().unwrap()
            {
                match code {
                    // Ctrl+C 处理
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        // 获取退出命令对应的 CommandSet
                        let exit_cmd = parse_exit();
                        // 检查并调用其 handle_enter 回调
                        if let Some(ref handler) = exit_cmd.handle_enter {
                            handler();
                        } else {
                            // 如果没有定义 handle_enter，则直接退出程序
                            std::process::exit(0);
                        }
                    }

                    // 字符输入（支持插入）
                    KeyCode::Char(c) => {
                        self.buffer.insert(self.cursor_position, c);
                        self.cursor_position += 1;
                        self.redraw_line(&mut stdout);
                    }

                    // 退格键（支持中间删除）
                    KeyCode::Backspace => {
                        if self.cursor_position > 0 {
                            self.buffer.remove(self.cursor_position - 1);
                            self.cursor_position -= 1;
                            self.redraw_line(&mut stdout);
                        }
                    }

                    // 方向键处理
                    KeyCode::Left => {
                        if self.cursor_position > 0 {
                            self.cursor_position -= 1;
                            self.update_cursor(&mut stdout);
                        }
                    }
                    KeyCode::Right => {
                        if self.cursor_position < self.buffer.len() {
                            self.cursor_position += 1;
                            self.update_cursor(&mut stdout);
                        }
                    }

                    // 上下方向键（保持原有历史逻辑）
                    KeyCode::Up => { /* 原有历史代码 */ }
                    KeyCode::Down => { /* 原有历史代码 */ }

                    // 回车键
                    KeyCode::Enter => {
                        write!(stdout, "\r\n").unwrap();
                        break;
                    }

                    _ => {}
                }
                stdout.flush().unwrap();
            }
        }

        execute!(stdout, cursor::Hide).unwrap(); // 隐藏光标
        self.buffer.clone()
    }

    // 重绘整行（用于插入/删除时）
    fn redraw_line(&mut self, stdout: &mut std::io::Stdout) {
        execute!(
            stdout,
            cursor::MoveToColumn(0),
            Clear(ClearType::CurrentLine)
        )
        .unwrap();
        write!(stdout, "ospfd> {}", self.buffer).unwrap();
        self.update_cursor(stdout);
    }

    // 更新光标位置（考虑提示符长度）
    fn update_cursor(&self, stdout: &mut std::io::Stdout) {
        let prompt_len = 7; // "ospfd> " 的长度
        execute!(
            stdout,
            cursor::MoveToColumn((prompt_len + self.cursor_position) as u16) // 这里缺少闭合括号
        )
        .unwrap(); // 补上这个括号
    }
}

pub fn main_loop() {
    terminal::enable_raw_mode().unwrap();
    //TODO CtrlC没写完
    //TODO /n -> /r/n

    let mut editor = LineEditor::new();
    loop {
        let line = editor.read_line();
        if line.trim().is_empty() {
            continue;
        }
        {
            let mut history = COMMAND_HISTORY.lock().unwrap();
            history.push_back(line.clone());
            if history.len() > MAX_HISTORY {
                history.pop_front();
            }
        }
        parse_cmd(line);
    }
    // 注意：由于主循环持续运行，此处通常不会退出。
    // terminal::disable_raw_mode().unwrap();
}
