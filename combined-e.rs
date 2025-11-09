
// File: YouRAM-master\codegen\src\attr.rs

0001: use syn::{parse::{Parse, ParseStream}, Expr, ExprField, Ident, LitStr, Member, ExprPath, Token};
0002: use quote::format_ident;
0003: 
0004: pub struct ModuleAttr {
0005:     pub ports: Vec<PortDefine>,
0006: }
0007: 
0008: // address: ("addr{address_width}", Input),
0009: pub struct PortDefine {
0010:     pub name: Ident,              // input / output
0011:     pub pattern: String,          // "A{n}"
0012:     pub direction: Ident,         // Input / Output / InOut
0013:     pub condition: Option<Expr> // e.g. "column_sel_size > 1"
0014: }
0015: 
0016: impl Parse for ModuleAttr {
0017:     fn parse(input: ParseStream) -> syn::Result<Self> {
0018:         let mut ports = Vec::new();
0019:         while !input.is_empty() {
0020:             let name: Ident = input.parse()?;
0021:             let _: Token![:] = input.parse()?;
0022:             let content;
0023:             syn::parenthesized!(content in input);
0024:             let pattern: LitStr = content.parse()?;
0025:             let _: Token![,] = content.parse()?;
0026:             let direction: Ident = content.parse()?;
0027: 
0028:             let condition = if content.peek(Token![,]) {
0029:                 let _: Token![,] = content.parse()?;
0030:                 let cond_str: LitStr = content.parse()?;
0031:                 let mut cond_expr: Expr = syn::parse_str(&cond_str.value())?;
0032:                 rewrite_condition_expr(&mut cond_expr);
0033:                 Some(cond_expr)
0034:             } else {
0035:                 None
0036:             };
0037: 
0038:             ports.push(PortDefine { name, pattern: pattern.value(), direction , condition});
0039: 
0040:             // optional trailing comma
0041:             let _ = input.parse::<Token![,]>();
0042:         }
0043:         Ok(ModuleAttr { ports })
0044:     }
0045: }
0046: 
0047: fn rewrite_condition_expr(expr: &mut Expr) {
0048:     match expr {
0049:         Expr::Binary(e) => {
0050:             rewrite_condition_expr(&mut *e.left);
0051:             rewrite_condition_expr(&mut *e.right);
0052:         }
0053:         Expr::Unary(e) => rewrite_condition_expr(&mut *e.expr),
0054:         Expr::Paren(e) => rewrite_condition_expr(&mut *e.expr),
0055:         Expr::Group(e) => rewrite_condition_expr(&mut *e.expr),
0056:         Expr::Path(expr_path) => {
0057:             if expr_path.qself.is_none() && expr_path.path.segments.len() == 1 {
0058:                 let ident = expr_path.path.segments[0].ident.clone();
0059:                 let name = ident.to_string();
0060: 
0061:                 if !matches!(name.as_str(), "module" | "self" | "crate" | "super") {
0062:                     let module_ident = format_ident!("module");
0063:                     let args_ident = format_ident!("args");
0064: 
0065:                     // 鏋勯€?module.args.column_sel_size
0066:                     let new_expr = Expr::Field(ExprField {
0067:                         attrs: Vec::new(),
0068:                         dot_token: Default::default(),
0069:                         member: Member::Named(ident),
0070:                         base: Box::new(Expr::Field(ExprField {
0071:                             attrs: Vec::new(),
0072:                             dot_token: Default::default(),
0073:                             member: Member::Named(args_ident),
0074:                             base: Box::new(Expr::Path(ExprPath {
0075:                                 attrs: Vec::new(),
0076:                                 qself: None,
0077:                                 path: module_ident.into(),
0078:                                 
0079:                             })),
0080:                         })),
0081:                     });
0082: 
0083:                     *expr = new_expr;
0084:                 }
0085:             }
0086:         }
0087:         Expr::Call(e) => {
0088:             rewrite_condition_expr(&mut *e.func);
0089:             for a in e.args.iter_mut() {
0090:                 rewrite_condition_expr(a);
0091:             }
0092:         }
0093:         Expr::MethodCall(e) => {
0094:             rewrite_condition_expr(&mut *e.receiver);
0095:             for a in e.args.iter_mut() {
0096:                 rewrite_condition_expr(a);
0097:             }
0098:         }
0099:         Expr::Index(e) => {
0100:             rewrite_condition_expr(&mut *e.expr);
0101:             rewrite_condition_expr(&mut *e.index);
0102:         }
0103:         Expr::Field(e) => rewrite_condition_expr(&mut *e.base),
0104:         Expr::Assign(e) => {
0105:             rewrite_condition_expr(&mut *e.left);
0106:             rewrite_condition_expr(&mut *e.right);
0107:         }
0108:         Expr::Range(e) => {
0109:             if let Some(from) = &mut e.start {
0110:                 rewrite_condition_expr(from);
0111:             }
0112:             if let Some(to) = &mut e.end {
0113:                 rewrite_condition_expr(to);
0114:             }
0115:         }
0116:         _ => {}
0117:     }
0118: }

// File: YouRAM-master\codegen\src\lib.rs

0001: mod attr;
0002: 
0003: use attr::ModuleAttr;
0004: use proc_macro::TokenStream;
0005: use quote::{quote, format_ident};
0006: use regex::Regex;
0007: use convert_case::{Case, Casing};
0008: 
0009: #[proc_macro_attribute]
0010: pub fn module(attr: TokenStream, item: TokenStream) -> TokenStream {
0011:     let input = syn::parse_macro_input!(item as syn::ItemStruct);
0012:     let attr = syn::parse_macro_input!(attr as ModuleAttr);
0013: 
0014:     let struct_name = &input.ident;
0015:     let struct_name_scase = struct_name.to_string().to_case(Case::Snake);
0016:     
0017:     let attrs = &input.attrs;
0018: 
0019:     let user_fields = match &input.fields {
0020:         syn::Fields::Named(fields_named) => &fields_named.named,
0021:         _ => panic!("Module struct must have named fields"),
0022:     };
0023: 
0024:     let arg_struct_name = format_ident!("{}Arg", struct_name);
0025: 
0026:     let mut port_name_functions = Vec::new();
0027:     let mut add_port_codes = Vec::new();
0028:     for port_define in attr.ports.iter() {
0029:         let function_name = format_ident!("{}_pn", port_define.name);
0030: 
0031:         let fields = extract_placeholders(&port_define.pattern);
0032:         match fields.len() {
0033:             0 => {
0034:                 let format_arg = port_define.pattern.clone();
0035:                 let code = quote! {
0036:                     pub fn #function_name() -> crate::circuit::ShrString {
0037:                         use crate::format_shr;
0038:                         format_shr!(#format_arg)
0039:                     }
0040:                 };
0041:                 port_name_functions.push(code);
0042: 
0043:                 let direction = &port_define.direction;
0044:                 let code = quote! {
0045:                     module.add_port(#struct_name::#function_name(), crate::circuit::PortDirection::#direction)?;
0046:                 };
0047:                 let code = if let Some(cond) = &port_define.condition {
0048:                     quote! {
0049:                         if #cond {
0050:                             #code
0051:                         }
0052:                     }
0053:                 } else {
0054:                     code
0055:                 };
0056: 
0057:                 add_port_codes.push(code);
0058:             }
0059:             1 => {
0060:                 let format_arg = port_define.pattern.clone();
0061:                 let field_ident = format_ident!("{}", fields[0]);
0062:                 let code = quote! {
0063:                     pub fn #function_name(#field_ident: usize) -> crate::circuit::ShrString {
0064:                         use crate::format_shr;
0065:                         format_shr!(#format_arg)
0066:                     }
0067:                 };
0068:                 port_name_functions.push(code);
0069: 
0070:                 let direction = &port_define.direction;
0071:                 let code = quote! {
0072:                     for v in 0..module.args.#field_ident {
0073:                         module.add_port(#struct_name::#function_name(v), crate::circuit::PortDirection::#direction)?;
0074:                     }
0075:                 };
0076:                 let code = if let Some(cond) = &port_define.condition {
0077:                     let code = quote! {
0078:                         if #cond {
0079:                             #code
0080:                         }
0081:                     };
0082:                     code
0083:                 } else {
0084:                     code
0085:                 };
0086: 
0087:                 
0088:                 add_port_codes.push(code);
0089:             }
0090:             2 => {
0091:                 let format_arg = port_define.pattern.clone();
0092:                 let f1 = format_ident!("{}", fields[0]);
0093:                 let f2 = format_ident!("{}", fields[1]);
0094:                 let direction = &port_define.direction;
0095:         
0096:                 let code = quote! {
0097:                     pub fn #function_name(#f1: usize, #f2: usize) -> crate::circuit::ShrString {
0098:                         use crate::format_shr;
0099:                         format_shr!(#format_arg)
0100:                     }
0101:                 };
0102:                 port_name_functions.push(code);
0103:         
0104:                 let mut code = quote! {
0105:                     for v1 in 0..module.args.#f1 {
0106:                         for v2 in 0..module.args.#f2 {
0107:                             module.add_port(
0108:                                 #struct_name::#function_name(v1, v2),
0109:                                 crate::circuit::PortDirection::#direction
0110:                             )?;
0111:                         }
0112:                     }
0113:                 };
0114:         
0115:                 if let Some(cond) = &port_define.condition {
0116:                     code = quote! {
0117:                         if #cond {
0118:                             #code
0119:                         }
0120:                     };
0121:                 }
0122:         
0123:                 add_port_codes.push(code);
0124:             }
0125:             _ => panic!("Unsupport"),
0126:         }
0127:     }
0128:     
0129:     let explicit_field_names: Vec<_> = user_fields.iter()
0130:         .filter(|f| f.attrs.is_empty())
0131:         .map(|f| &f.ident)
0132:         .collect();
0133: 
0134: 
0135:     let field_fmt: String = explicit_field_names
0136:         .iter()
0137:         .map(|_| "{}")
0138:         .collect::<Vec<_>>()
0139:         .join("_");
0140: 
0141: 
0142:     let format_string = format!("{}_{}", struct_name_scase, field_fmt);
0143:     let format_lit = syn::LitStr::new(&format_string, struct_name.span());
0144:     let self_fields: Vec<proc_macro2::TokenStream> = explicit_field_names
0145:         .iter()
0146:         .map(|ident| quote! { self.#ident })
0147:         .collect();
0148: 
0149:     quote! {
0150:         pub use derive_new::new;
0151:         #[derive(Debug, new)]
0152:         pub struct #arg_struct_name {
0153:             #user_fields
0154:         }
0155: 
0156:         #(#attrs)*
0157:         pub type #struct_name = crate::circuit::Module<#arg_struct_name>;
0158: 
0159:         impl crate::circuit::ModuleArg for #arg_struct_name {
0160:             fn create_module(self, factory: &mut crate::circuit::CircuitFactory) -> crate::YouRAMResult<#struct_name> {
0161:                 let name = self.module_name();
0162:                 let mut module = #struct_name::new(name, self);
0163:                 #(#add_port_codes)*
0164:                 module.build(factory)?;
0165:                 Ok(module)
0166:             }
0167: 
0168:             fn module_name(&self) -> crate::circuit::ShrString {
0169:                 use crate::format_shr;
0170:                 format_shr!(#format_lit, #(#self_fields),*)
0171:             }
0172:         }
0173: 
0174:         impl crate::circuit::Module<#arg_struct_name> {
0175:             #(#port_name_functions)*
0176:         }
0177:     }.into()
0178: }
0179: 
0180: 
0181: fn extract_placeholders(s: &str) -> Vec<String> {
0182:     let re = Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap();
0183:     re.captures_iter(s)
0184:         .map(|cap| cap[1].to_string())
0185:         .collect()
0186: }

// File: YouRAM-master\examples\create_circuit.rs

0001: #![allow(unused)]
0002: use std::{io::Write, sync::Arc};
0003: use tracing::Level;
0004: use youram::{circuit::{BankArg, BitcellArrayRecursiveArg, BufferArg, CircuitFactory, ControlLogic, ControlLogicArg, CoreArg, DataPathArg, DecoderArg, DriveStrength, FanoutBufferArg, SramArg}, export, pdk::Pdk, ErrorContext};
0005: 
0006: fn main_result() -> Result<(), Box<dyn std::error::Error>> {
0007:     tracing_subscriber::fmt()
0008:         .with_max_level(Level::INFO)
0009:         .with_target(false)
0010:         .with_file(false)
0011:         .with_line_number(false)
0012:         .init();
0013: 
0014:     let pdk = Arc::new(Pdk::load("./platforms/nangate45").context("load pdk")?);
0015:     let mut factory = CircuitFactory::new(pdk);
0016: 
0017:     // let logic = factory.module(ControlLogicArg::new())?;
0018:     // export::write_spice(logic, "./temp/logic.sp").context("export spice")?;
0019: 
0020:     // let decoder = factory.module(DecoderArg::new(10))?;
0021:     // export::write_spice(decoder, "./temp/decoder.sp").context("export spice")?;
0022: 
0023:     // let array = factory.module(BitcellArrayRecursiveArg::new(256, 256))?;
0024:     // export::write_spice(array, "./temp/bitcellarray.sp").context("export spice")?;
0025: 
0026:     // let fanout_buf = factory.module(FanoutBufferArg::new(256))?;
0027:     // export::write_spice(fanout_buf, "./temp/fanout_buf.sp").context("export spice")?;
0028: 
0029:     // let datapath = factory.module(DataPathArg::build(8, 4))?;
0030:     // export::write_spice(datapath, "./temp/datapath.sp").context("export spice")?;
0031: 
0032:     // let bank = factory.module(BankArg::new(16, 4, 4))?;
0033:     // export::write_spice(bank, "./temp/bank.sp").context("export spice")?;
0034: 
0035:     // let core = factory.module(CoreArg::new(16, 4, 4))?;
0036:     // export::write_spice(core, "./temp/core.sp").context("export spice")?;
0037: 
0038:     let sram = factory.module(SramArg::new(10, 8))?;
0039:     export::write_spice(sram, "./temp/sram.sp").context("export spice")?;
0040: 
0041:     std::io::stdout().flush()?;
0042:     Ok(())
0043: }
0044: 
0045: fn main() {
0046:     if let Err(e) = main_result() {
0047:         eprint!("Err: {}\n", e);
0048:     }
0049: }

// File: YouRAM-master\examples\spice_test.rs

0001: #![allow(unused)]
0002: use std::{io::Write, sync::Arc};
0003: use reda_unit::t;
0004: use tracing::Level;
0005: use tracing_subscriber::fmt::format;
0006: use youram::{circuit::{BankArg, BitcellArrayRecursiveArg, BufferArg, CircuitFactory, ControlLogic, ControlLogicArg, CoreArg, DataPathArg, Decoder, DecoderArg, DriveStrength, FanoutBufferArg, SramArg}, export, pdk::{Enviroment, Pdk}, simulate::{CircuitSimulator, NgSpice, VoltageAtMeas}, ErrorContext};
0007: 
0008: fn main_result() -> Result<(), Box<dyn std::error::Error>> {
0009:     tracing_subscriber::fmt()
0010:         .with_max_level(Level::INFO)
0011:         .with_target(false)
0012:         .with_file(false)
0013:         .with_line_number(false)
0014:         .init();
0015: 
0016:     let pdk = Arc::new(Pdk::load("./platforms/nangate45").context("load pdk")?);
0017:     let mut factory = CircuitFactory::new(pdk.clone());
0018:     let decoder = factory.module(DecoderArg::new(3))?;
0019: 
0020:     let pvt = pdk.pvt();
0021:     let env = Enviroment::new(pvt.clone(), t!(0.5 n), 0.0.into());
0022: 
0023:     let mut simulator = CircuitSimulator::create(
0024:         decoder.clone(), 
0025:         env, 
0026:         pdk.clone(), 
0027:         "./temp/simulate.sp", 
0028:         "./temp/decoder.sp"
0029:     )?;
0030: 
0031:     let decoder_ref = decoder.read();
0032:     simulator.write_logic1_stimulate(Decoder::address_pn(0));
0033:     simulator.write_logic1_stimulate(Decoder::address_pn(1));
0034:     simulator.write_logic1_stimulate(Decoder::address_pn(2));
0035: 
0036:     for i in 0..8 {
0037:         let meas = VoltageAtMeas::new(format!("output{i}"), Decoder::output_pn(i).to_string(), t!(10. n));
0038:         simulator.write_measurement(Box::new(meas))?;
0039:     }
0040:     simulator.write_trans(t!(0.5 n), t!(0.0), t!(15. n))?;
0041: 
0042: 
0043:     let result = simulator.simulate(&NgSpice, "./temp")?;
0044: 
0045:     for (name, value) in result {
0046:         println!("{name}: {value}");
0047:     }
0048: 
0049:     std::io::stdout().flush()?;
0050:     Ok(())
0051: }
0052: 
0053: fn main() {
0054:     if let Err(e) = main_result() {
0055:         eprint!("Err: {}\n", e);
0056:     }
0057: }

// File: YouRAM-master\src\error.rs

0001: use crate::{charz::CharzError, circuit::CircuitError, pdk::PdkError, simulate::SimulateError};
0002: 
0003: #[derive(Debug, thiserror::Error)]
0004: pub enum YouRAMError {
0005:     #[error(transparent)]
0006:     Io(#[from] std::io::Error),
0007: 
0008:     #[error(transparent)]
0009:     Fmt(#[from] std::fmt::Error),
0010: 
0011:     #[error(transparent)]
0012:     JsonError(#[from] serde_json::Error),
0013: 
0014:     #[error(transparent)]
0015:     Circuit(#[from] CircuitError),
0016: 
0017:     #[error(transparent)]
0018:     Simulate(#[from] SimulateError),
0019: 
0020:     #[error(transparent)]
0021:     Charz(#[from] CharzError),
0022: 
0023:     #[error(transparent)]
0024:     Pdk(#[from] PdkError),
0025: 
0026:     #[error("{0}")]
0027:     Message(String),
0028: 
0029:     #[error("{msg} >> {err}")]
0030:     Context { msg: String, err: Box<dyn std::error::Error> }
0031: }
0032: 
0033: pub type YouRAMResult<T> = Result<T, YouRAMError>;
0034: 
0035: pub trait ErrorContext<T> {
0036:     fn context<S: Into<String>>(self, msg: S) -> YouRAMResult<T>;
0037:     fn with_context<S: Into<String>>(self, f: impl Fn() -> S) -> YouRAMResult<T>;
0038: }
0039: 
0040: impl<T, E: std::error::Error + 'static> ErrorContext<T> for Result<T, E> {
0041:     fn context<S: Into<String>>(self, msg: S) -> YouRAMResult<T> {
0042:         self.map_err(|e| YouRAMError::Context { msg: msg.into(), err: Box::new(e) }) 
0043:     }
0044: 
0045:     fn with_context<S: Into<String>>(self, f: impl Fn() -> S) -> YouRAMResult<T> {
0046:         let msg = f();
0047:         self.context(msg)
0048:     }
0049: }

// File: YouRAM-master\src\lib.rs

0001: #![feature(mapped_lock_guards)]
0002: pub mod circuit;
0003: pub mod pdk;
0004: pub mod simulate;
0005: pub mod charz;
0006: pub mod export;
0007: pub mod error;
0008: pub use error::*;
0009: 
0010: pub use derive_new;

// File: YouRAM-master\src\main.rs

0001: use std::{path::{Path, PathBuf}, sync::Arc};
0002: use reda_unit::{t, Time};
0003: use serde::{Deserialize, Serialize};
0004: use tracing::{info, Level};
0005: use clap::Parser;
0006: use youram::{
0007:     charz::{FunctionCharz, FunctionCharzPolicy, RandomPolicy}, 
0008:     circuit::{CircuitFactory, SramArg}, 
0009:     export, 
0010:     pdk::{Enviroment, Pdk}, 
0011:     simulate::{SpiceCommand, NgSpice}, 
0012:     ErrorContext
0013: };
0014: 
0015: fn main_result() -> Result<(), Box<dyn std::error::Error>> {   
0016:     let args = Args::parse();
0017:     tracing_subscriber::fmt()
0018:         .with_max_level(args.level())
0019:         .with_target(false)
0020:         .with_file(false)
0021:         .with_line_number(false)
0022:         .init();
0023: 
0024:     // load config
0025:     let config: Config = {
0026:         let context = std::fs::read_to_string(&args.config).context("read config file")?;
0027:         serde_json::from_str(&context).context("parse config file")?
0028:     };
0029:     config.create_output_path()?;
0030: 
0031:     // load pdk
0032:     let pdk = Arc::new(Pdk::load(&config.pdk_path).context("load pdk")?);
0033:     
0034:     // create sram
0035:     let mut factory = CircuitFactory::new(pdk.clone());
0036:     let sram = factory.module(SramArg::new(config.address_width, config.word_width)).context("create sram")?;
0037: 
0038:     // test sram
0039:     if let Some(function_test) = &config.function_test {
0040:         let policy = parse_function_test_policy(&function_test)?;
0041:         let period = config.period;
0042:         let command = config.spice_command()?;
0043: 
0044:         // load simulate config in pdk
0045:         let pvt = pdk.pvt();
0046:         let output_load = pdk.default_fanout_load().unwrap_or(0.0.into());
0047:         let input_slew = period / 20.0;
0048:         let env = Enviroment::new(pvt.clone(), input_slew, output_load);
0049: 
0050:         // run functional test
0051:         FunctionCharz::config()
0052:             .sram(sram.clone())
0053:             .period(period)
0054:             .env(env)
0055:             .pdk(pdk.clone())
0056:             .policy_box(policy)
0057:             .command_box(command)
0058:             .temp_folder(config.temp_folder_path())
0059:             .test()?;
0060:     }
0061: 
0062:     // write
0063:     if config.export_spice {
0064:         let spice_file = config.join_output(format!("{}.sp", sram.read().name));
0065:         export::write_spice(sram.clone(), spice_file)?;
0066:     }
0067:     
0068:     if config.export_verilog {
0069:         let verilog_file = config.join_output(format!("{}.v", sram.read().name));
0070:         export::write_verilog(sram.clone(), verilog_file)?;
0071:     }
0072: 
0073:     if config.export_liberty {
0074:         let liberty_file = config.join_output(format!("{}.lib", sram.read().name));
0075:         let command = config.spice_command()?;
0076:         export::write_liberty(
0077:             sram.clone(), 
0078:             liberty_file, 
0079:             config.period, 
0080:             pdk.clone(), 
0081:             command, 
0082:             config.temp_folder_path()
0083:         )?;
0084:     }
0085: 
0086:     Ok(())
0087: }
0088: 
0089: fn main() {
0090:     if let Err(e) = main_result() {
0091:         eprint!("Err: {}\n", e);
0092:     }
0093: }
0094: 
0095: /// A simple example CLI
0096: #[derive(Parser, Debug)]
0097: #[command(name = "youram")]
0098: #[command(about = "A Sram Compiler", long_about = None)]
0099: struct Args {
0100:     /// Path to the configuration file
0101:     #[arg(short, long)]
0102:     config: String,
0103: 
0104:     /// Enable verbose output
0105:     #[arg(short, long)]
0106:     verbose: bool,
0107: }
0108: 
0109: impl Args {
0110:     pub fn level(&self) -> Level {
0111:         if self.verbose { Level::DEBUG } else { Level::INFO }
0112:     }
0113: }
0114: 
0115: #[derive(Debug, Deserialize, Serialize)]
0116: pub struct Config {
0117:     pub pdk_path: PathBuf,
0118:     pub output_path: PathBuf,
0119: 
0120:     pub address_width: usize,
0121:     pub word_width: usize,
0122:     
0123:     #[serde(default = "default_spice_command")]
0124:     pub spice_command: String,
0125: 
0126:     #[serde(default = "default_period")]
0127:     pub period: Time,
0128:     
0129:     pub function_test: Option<String>,
0130: 
0131:     #[serde(default = "const_true")]
0132:     pub export_spice: bool,
0133: 
0134:     #[serde(default = "const_true")]
0135:     pub export_verilog: bool,
0136: 
0137:     #[serde(default = "const_false")]
0138:     pub export_liberty: bool,
0139: }
0140: 
0141: fn parse_function_test_policy(policy: &str) -> Result<Box<dyn FunctionCharzPolicy>, Box<dyn std::error::Error>> {
0142:     match policy {
0143:         "random" => Ok(Box::new(RandomPolicy)),
0144:         _ => Err(format!("Un support function test policy: {}", policy))?,
0145:     }
0146: }
0147: 
0148: impl Config {
0149:     pub fn create_output_path(&self) -> Result<(), Box<dyn std::error::Error>> {
0150:         // Ensure output directory exists
0151:         if !self.output_path.exists() {
0152:             std::fs::create_dir_all(&self.output_path)?;
0153:             info!("created output directory: {:?}", self.output_path);
0154:         }
0155: 
0156:         // Ensure temp directory exists
0157:         let temp_path = self.temp_folder_path();
0158:         if !temp_path.exists() {
0159:             std::fs::create_dir_all(&temp_path)?;
0160:             info!("created temp directory: {:?}", temp_path);
0161:         }
0162: 
0163:         Ok(())
0164:     }
0165: 
0166:     pub fn temp_folder_path(&self) -> PathBuf {
0167:         self.output_path.join("temp")
0168:     }
0169: 
0170:     pub fn join_output(&self, path: impl AsRef<Path>) -> PathBuf {
0171:         self.output_path.join(path.as_ref())
0172:     }
0173: 
0174:     pub fn spice_command(&self) -> Result<Box<dyn SpiceCommand>, Box<dyn std::error::Error>> {
0175:         match self.spice_command.as_str() {
0176:             "ngspice" => Ok(Box::new(NgSpice)),
0177:             _ => Err(format!("Un support spice executor: {}", self.spice_command.as_str()))?,
0178:         }
0179:     }
0180: }
0181: 
0182: fn default_spice_command() -> String {
0183:     "ngspice".to_string()
0184: }
0185: 
0186: fn default_period() -> Time {
0187:     t!(10 n)
0188: }
0189: 
0190: const fn const_true() -> bool {
0191:     true
0192: }
0193: 
0194: const fn const_false() -> bool {
0195:     false
0196: }

// File: YouRAM-master\src\charz\error.rs

0001: 
0002: #[derive(Debug, thiserror::Error)]
0003: pub enum CharzError {
0004:     #[error("lack function test config {0}")]
0005:     LackFunctionTestConfigField(&'static str),
0006: }

// File: YouRAM-master\src\charz\mod.rs

0001: mod error;
0002: mod transaction;
0003: mod function;
0004: mod timing;
0005: 
0006: pub use error::*;
0007: pub use transaction::*;
0008: pub use function::*;
0009: pub use timing::*;

// File: YouRAM-master\src\charz\transaction.rs

0001: use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};
0002: use rand::Rng;
0003: use reda_unit::{t, v, Number, Time, Voltage};
0004: use tracing::warn;
0005: use crate::{circuit::{Shr, ShrString, Sram}, export, pdk::{Enviroment, Pdk}, simulate::{CircuitSimulator, Meas, SpiceCommand}, ErrorContext, YouRAMResult};
0006: pub type Bits = Vec<bool>;
0007: 
0008: pub enum SramTransaction {
0009:     Write { address: Bits, word: Bits },
0010:     Read { address: Bits },
0011: }
0012: 
0013: /// Generate SRAM transaction and meas in logic.
0014: /// 
0015: /// Call `simulate` method to run spice simulate and get meas result
0016: /// 
0017: /// Clock set: 
0018: ///
0019: ///   transaction 1
0020: ///         |
0021: ///         v
0022: ///         +---+   +---+   +---+
0023: /// clk:    |   |   |   |   |   |
0024: ///     +---+   +---+   +---+   +---
0025: ///     ^       ^
0026: ///     |       |
0027: ///    t = 0   t = period
0028: ///
0029: ///
0030: pub struct SramTransactionGenerator {
0031:     pub sram: Shr<Sram>,
0032:     pub period: Time,
0033: 
0034:     transactions: Vec<SramTransaction>,
0035:     read_transaction_size: usize,
0036:     write_transaction_size: usize,
0037:     measurements: Vec<Box<dyn Meas>>, 
0038:     memory: HashMap<usize, Vec<bool>>,
0039:     addr_mask: usize,
0040:     word_mask: usize,
0041:     max_address: usize,
0042:     max_word: usize,
0043: }
0044: 
0045: impl SramTransactionGenerator {
0046:     pub fn new(sram: Shr<Sram>, period: Time) -> Self {
0047:         let addr_mask = Self::full_bits_number(sram.read().address_width());
0048:         let word_mask = Self::full_bits_number(sram.read().word_width());
0049:         let max_address = 2usize.pow(sram.read().address_width() as u32) - 1;
0050:         let max_word = 2usize.pow(sram.read().word_width() as u32) - 1;
0051:         
0052:         Self {
0053:             sram,
0054:             period,
0055:             transactions: vec![],
0056:             read_transaction_size: 0,
0057:             write_transaction_size: 0,
0058:             measurements: vec![],
0059:             memory: HashMap::new(),
0060:             addr_mask,
0061:             word_mask,
0062:             max_address,
0063:             max_word,
0064:         }
0065:     }
0066: 
0067:     pub fn simulate(
0068:         self,
0069:         env: Enviroment,
0070:         pdk: Arc<Pdk>,
0071:         command: &impl SpiceCommand,
0072:         simulate_path: impl Into<PathBuf>,
0073:         circuit_path: Option<impl Into<PathBuf>>, 
0074:         temp_folder: impl AsRef<Path>,
0075:     ) -> YouRAMResult<HashMap<String, Number>>  {
0076:         // write sram if need
0077:         let temp_folder = temp_folder.as_ref();
0078:         let circuit_path = match circuit_path {
0079:             Some(circuit_path) => circuit_path.into(),
0080:             None => {
0081:                 let circuit_path = temp_folder.join(self.sram.read().name.to_string());
0082:                 export::write_spice(self.sram.clone(), &circuit_path).with_context(|| format!("write sram"))?;
0083:                 circuit_path
0084:             }
0085:         };
0086: 
0087:         let mut simulator = CircuitSimulator::create(self.sram.clone(), env, pdk, simulate_path, circuit_path)?;
0088:     
0089:         // transform logic transactions to real voltages values
0090:         let mut we_voltags = vec![];
0091:         let mut address_voltags = vec![vec![]; self.sram.read().address_width()];
0092:         let mut word_voltags = vec![vec![]; self.sram.read().word_width()];
0093: 
0094:         for transaction in self.transactions.iter() {
0095:             match transaction {
0096:                 SramTransaction::Write { address, word } => {
0097:                     we_voltags.push(simulator.logic1_voltage());
0098:                     
0099:                     for (voltags, &value) in address_voltags.iter_mut().zip(address) {
0100:                         voltags.push(simulator.logic_voltage(value));
0101:                     }
0102: 
0103:                     for (voltags, &value) in word_voltags.iter_mut().zip(word) {
0104:                         voltags.push(simulator.logic_voltage(value));
0105:                     }
0106: 
0107:                 }
0108:                 SramTransaction::Read { address } => {
0109:                     we_voltags.push(v!(0));
0110:                     for (voltags, &value) in address_voltags.iter_mut().zip(address) {
0111:                         voltags.push(simulator.logic_voltage(value));
0112:                     }
0113: 
0114:                     for voltags in word_voltags.iter_mut() {
0115:                         voltags.push(v!(0.));
0116:                     }
0117:                 }
0118:             }
0119:         }
0120: 
0121:         // write inputs
0122:         simulator.write_clock(self.period)?;
0123: 
0124:         let mut write_stimulation = |port_name: ShrString, voltages: &[Voltage]| -> YouRAMResult<()> {
0125:             simulator.write_period_stimulate(port_name, voltages, self.period, 0.0)
0126:         };
0127:             
0128:         write_stimulation(Sram::chip_sel_bar_pn(), &[v!(0.)])?;
0129:         write_stimulation(Sram::write_enable_pn(), &we_voltags)?;
0130:         
0131:         for (i, address) in address_voltags.iter().enumerate() {
0132:             write_stimulation(Sram::address_pn(i), address)?;
0133:         }
0134: 
0135:         for (i, word) in word_voltags.iter().enumerate() {
0136:             write_stimulation(Sram::data_input_pn(i), word)?;
0137:         }
0138: 
0139:         // write meas
0140:         for meas in self.measurements {
0141:             simulator.write_measurement(meas)?;
0142:         }
0143: 
0144:         // write trans
0145:         let end_time = self.period * (self.transactions.len() + 2) as f64;
0146:         simulator.write_trans(t!(10 p), 0.0, end_time)?;
0147: 
0148:         // run simulate
0149:         simulator.simulate(command, temp_folder)
0150:     }
0151: 
0152:     pub fn add_random_write_transaction(&mut self) -> bool {
0153:         let address = self.random_address();
0154:         let word = self.random_word();
0155:         self.add_write_transaction(address, word)
0156:     }
0157: 
0158:     pub fn add_random_read_transaction(&mut self) -> bool {
0159:         let address = self.random_address();
0160:         self.add_read_transaction(address)
0161:     }
0162: 
0163:     /// Add a read transaction, and update sram memory state
0164:     pub fn add_write_transaction(&mut self, address: usize, word: usize) -> bool {
0165:         let address: usize = self.mask_address(address);
0166:         let word = self.mask_word(word);
0167: 
0168:         self.transactions.push(SramTransaction::write(
0169:             self.address_to_bits(address), 
0170:             self.word_to_bits(word)
0171:         ));
0172:         self.write_transaction_size += 1;
0173: 
0174:         self.memory.insert(address, self.word_to_bits(word));
0175: 
0176:         true
0177:     }
0178: 
0179:     /// Add a read transaction
0180:     /// if address not writed yet, return false
0181:     pub fn add_read_transaction(&mut self, address: usize) -> bool {
0182:         let address: usize = self.mask_address(address);
0183: 
0184:         if !self.memory.contains_key(&address) {
0185:             warn!("try to read an unset address 0x{0:x}, this transaction will be ignored.", address);
0186:             return false;
0187:         }
0188: 
0189:         self.transactions.push(SramTransaction::read(self.address_to_bits(address)));
0190:         self.read_transaction_size += 1;
0191: 
0192:         true
0193:     } 
0194: 
0195:     pub fn add_measurement<M: Meas + 'static>(&mut self, meas: impl Into<Box<M>>) {
0196:         self.measurements.push(meas.into());
0197:     }
0198: 
0199:     pub fn clock_rise_time(&self, clock_index: usize) -> Time {
0200:         clock_index as f64 * self.period + self.period / 2.
0201:     }
0202: 
0203:     pub fn last_clock_rise_time(&self) -> Time {
0204:         self.clock_rise_time(self.transactions.len() - 1)
0205:     }
0206: 
0207:     pub fn half_period(&self) -> Time {
0208:         self.period / 2.0
0209:     }
0210: 
0211:     pub fn clock_begin(&self, clock_index: usize) -> Time {
0212:         clock_index as f64 * self.period
0213:     }
0214: 
0215:     pub fn read_transaction_size(&self) -> usize {
0216:         self.read_transaction_size
0217:     }
0218: 
0219:     pub fn write_transaction_size(&self) -> usize {
0220:         self.write_transaction_size
0221:     }
0222: 
0223:     pub fn memory(&self, address: usize) -> Option<&Bits> {
0224:         self.memory.get(&address)
0225:     }
0226: 
0227:     pub fn transaction_size(&self) -> usize {
0228:         self.transactions.len()
0229:     }
0230: 
0231:     #[inline]
0232:     fn address_to_bits(&self, address: usize) -> Bits {
0233:         Self::usize_to_bits(address, self.sram.read().args.address_width)
0234:     }
0235: 
0236:     #[inline]
0237:     fn word_to_bits(&self, word: usize) -> Bits {
0238:         Self::usize_to_bits(word, self.sram.read().args.word_width)
0239:     }
0240: 
0241:     #[inline]
0242:     pub fn max_address(&self) -> usize {
0243:         self.max_address
0244:     }
0245: 
0246:     #[inline]
0247:     pub fn max_word(&self) -> usize {
0248:         self.max_word
0249:     }
0250: 
0251:     #[inline]
0252:     pub fn mask_address(&self, address: usize) -> usize {
0253:         self.addr_mask & address
0254:     }
0255: 
0256:     #[inline]
0257:     pub fn mask_word(&self, word: usize) -> usize {
0258:         self.word_mask & word
0259:     }
0260: 
0261:     #[inline]
0262:     pub fn random_address(&self) -> usize {
0263:         Self::random_usize(self.max_address)  
0264:     }
0265: 
0266:     #[inline]
0267:     pub fn random_word(&self) -> usize {
0268:         Self::random_usize(self.max_word)
0269:     }
0270: 
0271:     /// generate a usize in range [0, max]
0272:     fn random_usize(max: usize) -> usize {
0273:         let mut rng = rand::rng();
0274:         rng.random_range(0..=max)
0275:     }
0276: 
0277:     fn usize_to_bits(mut value: usize, size: usize) -> Bits {
0278:         let mut bits = vec![false; size];
0279:         for i in 0..size {
0280:             let bit = 0 != (value & 0x00000001usize);
0281:             bits[i] = bit;
0282:             value >>= 1;
0283:         }
0284: 
0285:         bits
0286:     }
0287: 
0288:     fn full_bits_number(size: usize) -> usize {
0289:         let mut value = 0usize;
0290:         for _ in 0..size {
0291:             value = (value << 1) + 1;
0292:         } 
0293:         value
0294:     }
0295:     
0296: }
0297: 
0298: impl SramTransaction {
0299:     pub fn write(address: impl Into<Bits>, word: impl Into<Bits>) -> Self {
0300:         Self::Write { address: address.into(), word: word.into() }
0301:     }
0302: 
0303:     pub fn read(address: impl Into<Bits>) -> Self {
0304:         Self::Read { address: address.into() }
0305:     }
0306: }

// File: YouRAM-master\src\charz\function\marchc.rs

0001: use crate::YouRAMResult;
0002: use super::{FunctionCharzPolicy, FunctionTransactionGenerator};
0003: 
0004: /// March_algorithm: https://en.wikipedia.org/wiki/March_algorithm
0005: pub struct MarchCPolicy;
0006: 
0007: impl FunctionCharzPolicy for MarchCPolicy {
0008:     fn generate_transactions(&self, charz: &mut FunctionTransactionGenerator) -> YouRAMResult<()> {
0009:         let max_address = charz.transactions.max_address();
0010:         let full_word = charz.transactions.max_word();
0011:     
0012:         // Write 0 from low address to high address
0013:         for address in 0..=max_address {
0014:             charz.add_write_transaction(address, 0);
0015:         }
0016:     
0017:         // Read 0 and write 1 from low address to high address
0018:         for address in 0..=max_address {
0019:             charz.add_read_transaction(address);
0020:             charz.add_write_transaction(address, full_word);
0021:         }
0022:     
0023:         // Read 1 and write 0 from low address to high address
0024:         for address in 0..=max_address {
0025:             charz.add_read_transaction(address);
0026:             charz.add_write_transaction(address, 0);
0027:         }
0028:     
0029:         // Read 0 from low address to high address
0030:         for address in 0..=max_address {
0031:             charz.add_read_transaction(address);
0032:         }
0033:     
0034:         // Read 0 and write 1 from high address to low address
0035:         for address in (0..=max_address).rev() {
0036:             charz.add_read_transaction(address);
0037:             charz.add_write_transaction(address, full_word);
0038:         }
0039:     
0040:         // Read 1 and write 0 from high address to low address
0041:         for address in (0..=max_address).rev() {
0042:             charz.add_read_transaction(address);
0043:             charz.add_write_transaction(address, 0);
0044:         }
0045:     
0046:         // Read 0 from high address to low address
0047:         for address in (0..=max_address).rev() {
0048:             charz.add_read_transaction(address);
0049:         }
0050:     
0051:         Ok(())
0052:     }
0053:     
0054: }

// File: YouRAM-master\src\charz\function\marchcminus.rs

0001: use crate::YouRAMResult;
0002: use super::{FunctionCharzPolicy, FunctionTransactionGenerator};
0003: 
0004: pub struct MarchCMinusPolicy;
0005: 
0006: impl FunctionCharzPolicy for MarchCMinusPolicy {
0007:     fn generate_transactions(&self, charz: &mut FunctionTransactionGenerator) -> YouRAMResult<()> {
0008:         let max_address = charz.transactions.max_address();
0009:         let full_word = charz.transactions.max_word();
0010:     
0011:         // Write 0 from low address to high address
0012:         for address in 0..=max_address {
0013:             charz.add_write_transaction(address, 0);
0014:         }
0015:     
0016:         // Read 0 and write 1 from low address to high address
0017:         for address in 0..=max_address {
0018:             charz.add_read_transaction(address);
0019:             charz.add_write_transaction(address, full_word);
0020:         }
0021:     
0022:         // Read 1 and write 0 from low address to high address
0023:         for address in 0..=max_address {
0024:             charz.add_read_transaction(address);
0025:             charz.add_write_transaction(address, 0);
0026:         }
0027:     
0028:         // Read 0 and write 1 from high address to low address
0029:         for address in (0..=max_address).rev() {
0030:             charz.add_read_transaction(address);
0031:             charz.add_write_transaction(address, full_word);
0032:         }
0033:     
0034:         // Read 1 and write 0 from high address to low address
0035:         for address in (0..=max_address).rev() {
0036:             charz.add_read_transaction(address);
0037:             charz.add_write_transaction(address, 0);
0038:         }
0039:     
0040:         // Read 0 from high address to low address
0041:         for address in (0..=max_address).rev() {
0042:             charz.add_read_transaction(address);
0043:         }
0044:     
0045:         Ok(())
0046:     }
0047:     
0048: }

// File: YouRAM-master\src\charz\function\marchx.rs

0001: use crate::YouRAMResult;
0002: use super::{FunctionCharzPolicy, FunctionTransactionGenerator};
0003: 
0004: pub struct MarchXPolicy;
0005: 
0006: impl FunctionCharzPolicy for MarchXPolicy {
0007:     fn generate_transactions(&self, charz: &mut FunctionTransactionGenerator) -> YouRAMResult<()> {
0008:         let max_address = charz.transactions.max_address();
0009:         let full_word = charz.transactions.max_word();
0010:     
0011:         // Write 0 from low address to high address
0012:         for address in 0..=max_address {
0013:             charz.add_write_transaction(address, 0);
0014:         }
0015:     
0016:         // Read 0 and write 1 from low address to high address
0017:         for address in 0..=max_address {
0018:             charz.add_read_transaction(address);
0019:             charz.add_write_transaction(address, full_word);
0020:         }
0021:     
0022:         // Read 1 and write 0 from high address to low address
0023:         for address in (0..=max_address).rev() {
0024:             charz.add_read_transaction(address);
0025:             charz.add_write_transaction(address, 0);
0026:         }
0027:     
0028:         // Read 0 from high address to low address
0029:         for address in (0..=max_address).rev() {
0030:             charz.add_read_transaction(address);
0031:         }
0032:     
0033:         Ok(())
0034:     }
0035:     
0036: }

// File: YouRAM-master\src\charz\function\mats.rs

0001: use crate::YouRAMResult;
0002: use super::{FunctionCharzPolicy, FunctionTransactionGenerator};
0003: 
0004: pub struct MatSPolicy;
0005: 
0006: impl FunctionCharzPolicy for MatSPolicy {
0007:     fn generate_transactions(&self, charz: &mut FunctionTransactionGenerator) -> YouRAMResult<()> {
0008:         let max_address = charz.transactions.max_address();
0009:         let full_word = charz.transactions.max_word();
0010:     
0011:         // Write 0 from low address to high address
0012:         for address in 0..=max_address {
0013:             charz.add_write_transaction(address, 0);
0014:         }
0015:     
0016:         // Read 0 and write 1 from low address to high address
0017:         for address in 0..=max_address {
0018:             charz.add_read_transaction(address);
0019:             charz.add_write_transaction(address, full_word);
0020:         }
0021:     
0022:         // Read 1 from high address to low address
0023:         for address in (0..=max_address).rev() {
0024:             charz.add_read_transaction(address);
0025:         }
0026:     
0027:         Ok(())
0028:     }
0029: }

// File: YouRAM-master\src\charz\function\mod.rs

0001: mod random;
0002: mod mats;
0003: mod marchx;
0004: mod marchcminus;
0005: mod marchc;
0006: pub use random::*;
0007: pub use mats::*;
0008: pub use marchx::*;
0009: pub use marchcminus::*;
0010: pub use marchc::*;
0011: 
0012: use std::{collections::HashMap, path::PathBuf, sync::Arc};
0013: use approx::AbsDiffEq;
0014: use reda_unit::{t, Number, Time, Voltage};
0015: use tracing::{debug, error, info, warn};
0016: use crate::{circuit::{Shr, Sram}, pdk::{Enviroment, Pdk}, simulate::{NgSpice, SpiceCommand, VoltageAtMeas}, YouRAMResult};
0017: use super::{CharzError, SramTransactionGenerator};
0018: 
0019: /// Function charz for Sram
0020: /// 
0021: /// # Deafult:
0022: /// - policy: random 
0023: /// - command: ngspice
0024: /// - temp_folder: "./temp"
0025: /// - simulate_path: "./temp/simulator.sp"
0026: /// - circuit_path: "./temp/<sram_name>.sp"
0027: /// 
0028: /// # Example
0029: /// 
0030: /// ```no_run
0031: /// let result = FunctionCharz::config()
0032: ///     .sram(sram)
0033: ///     .policy(RandomPolicy)
0034: ///     .command(NgSpice)
0035: ///     .pdk(pdk)
0036: ///     .....
0037: ///     .period(t!(10 n))
0038: ///     .analyze()?;
0039: /// ```
0040: pub struct FunctionCharz {
0041:     pub sram: Option<Shr<Sram>>,
0042:     pub period: Option<Time>,
0043:     pub env: Option<Enviroment>,
0044:     pub pdk: Option<Arc<Pdk>>,
0045:     
0046:     pub policy: Option<Box<dyn FunctionCharzPolicy>>,
0047:     pub command: Option<Box<dyn SpiceCommand>>,
0048: 
0049:     pub temp_folder: Option<PathBuf>,
0050:     pub simulate_path: Option<PathBuf>,
0051:     pub circuit_path: Option<PathBuf>,
0052: }
0053: 
0054: impl FunctionCharz {
0055:     pub fn test(self) -> YouRAMResult<bool> {
0056:         info!("execute function charz");
0057: 
0058:         // extract args
0059:         debug!("extract arguments");
0060:         let sram = self.sram.ok_or(CharzError::LackFunctionTestConfigField("sram"))?;
0061:         let period = self.period.ok_or(CharzError::LackFunctionTestConfigField("period"))?;
0062:         let env = self.env.ok_or(CharzError::LackFunctionTestConfigField("env"))?;
0063:         let pdk = self.pdk.ok_or(CharzError::LackFunctionTestConfigField("pdk"))?;
0064:         let policy = self.policy.ok_or(CharzError::LackFunctionTestConfigField("policy"))?;
0065: 
0066:         let command = self.command.ok_or(CharzError::LackFunctionTestConfigField("command"))?;
0067: 
0068:         let temp_folder =  self.temp_folder.unwrap_or_else(|| "./temp".into());
0069:         let simulate_path = self.simulate_path.unwrap_or_else(|| temp_folder.join("simulate.sp"));
0070:         let circuit_path = self.circuit_path;
0071: 
0072:         // generate transactions to test
0073:         debug!("generate transactions");
0074:         let mut transactions = FunctionTransactionGenerator::new(sram.clone(), period);
0075:         policy.generate_transactions(&mut transactions)?;
0076:         
0077:         // execuate spice simulate
0078:         debug!("spice simulate");
0079:         let voltage = env.voltage();
0080:         let result = transactions.transactions.simulate(env, pdk, &command, simulate_path, circuit_path, temp_folder)?;
0081:         let expect_result = transactions.target_meas_result;
0082:     
0083:         // check simulation result
0084:         debug!("check simulation result");
0085:         Self::check_result(&expect_result, result, voltage)
0086:     }
0087: 
0088:     fn check_result(expect_result: &HashMap<String, bool>, result: HashMap<String, Number>, voltage: Voltage) -> YouRAMResult<bool> {
0089:         let mut failed_size = 0;
0090:         for (name, target_value) in expect_result.iter() {
0091:             debug!("check meas {}", name);
0092:             match result.get(name) {
0093:                 None => {
0094:                     error!("the .meas task {} not found!", name);
0095:                     return Ok(false);
0096:                 }
0097:                 Some(real_value) => {
0098:                     let target_value = if *target_value { voltage } else { 0.0.into() }; 
0099:                     if !target_value.to_f64().abs_diff_eq(&real_value.to_f64(), 1e-2) {
0100:                         debug!("check meas {} failed, expect {}, not got {}", name, target_value, real_value);
0101:                         failed_size += 1;
0102:                     }
0103:                 }
0104:             }
0105:         }
0106:     
0107:         if failed_size == 0 {
0108:             info!("functional test pass in all {} test",expect_result.len());
0109:             Ok(true)
0110:         } else {
0111:             warn!("functional test failed, {} failed in {} test", failed_size, expect_result.len());
0112:             Ok(false)
0113:         }
0114:     }
0115: }
0116: 
0117: pub struct FunctionTransactionGenerator {
0118:     pub transactions: SramTransactionGenerator,
0119:     pub target_meas_result: HashMap<String, bool>, 
0120: }
0121: 
0122: pub trait FunctionCharzPolicy {
0123:     fn generate_transactions(&self, charz: &mut FunctionTransactionGenerator) -> YouRAMResult<()>;
0124: }
0125: 
0126: impl FunctionTransactionGenerator {
0127:     pub fn new(sram: Shr<Sram>, period: Time) -> Self {
0128:         Self { 
0129:             transactions: SramTransactionGenerator::new(sram, period),
0130:             target_meas_result: HashMap::new()
0131:         }
0132:     }
0133: 
0134:     pub fn generate_transactions(&mut self, policy: impl FunctionCharzPolicy) -> YouRAMResult<()> {
0135:         policy.generate_transactions(self)
0136:     }
0137: 
0138:     pub fn add_write_transaction(&mut self, address: usize, word: usize) {
0139:         self.transactions.add_write_transaction(address, word);
0140:     }
0141: 
0142:     pub fn add_read_transaction(&mut self, address: usize) {
0143:         if !self.transactions.add_read_transaction(address) {
0144:             return;
0145:         }
0146: 
0147:         let bits = self.transactions.memory(address).unwrap().clone();
0148:         // Wow, the last transaction is read, if there is `size` transactions
0149:         // This read transaction's index is `transaction-1`, it will be enbale by `transaction-1` clock
0150:         // So, we can read output in No. `transaction`'s clock rise
0151:         let meas_time = self.transactions.clock_rise_time(self.transactions.transaction_size()) - t!(1 n);
0152: 
0153:         // for each bit of ouput port, add a meas
0154:         for (bit_index, &bit) in bits.iter().enumerate() {
0155:             let meas_index = self.target_meas_result.len();
0156:             let meas_name = format!("dout{}_{}", bit_index, meas_index);
0157:             let port_name = Sram::data_output_pn(bit_index);        
0158:             let meas = VoltageAtMeas::new(meas_name.clone(), port_name.to_string(), meas_time);
0159: 
0160:             self.transactions.add_measurement(meas);
0161:             self.target_meas_result.insert(meas_name, bit);
0162:         } 
0163:     }
0164: 
0165: }
0166: 
0167: impl Default for FunctionCharz {
0168:     fn default() -> Self {
0169:         Self {
0170:             sram: None,
0171:             period: None,
0172:             env: None, 
0173:             pdk: None,
0174:             policy: Some(Box::new(RandomPolicy)),
0175:             command: Some(Box::new(NgSpice)),
0176:             temp_folder: Some("./temp".into()),
0177:             simulate_path: None, 
0178:             circuit_path: None,
0179:         }
0180:     }
0181: }
0182: 
0183: impl FunctionCharz {
0184:     pub fn config() -> Self {
0185:         Self::default()
0186:     }
0187: 
0188:     pub fn sram(self, sram: impl Into<Shr<Sram>>) -> Self {
0189:         let mut build = self;
0190:         build.sram = Some(sram.into());
0191:         build
0192:     }
0193: 
0194:     pub fn period(self, period: impl Into<Time>) -> Self {
0195:         let mut build = self;
0196:         build.period = Some(period.into());
0197:         build
0198:     }
0199: 
0200:     pub fn env(self, env: impl Into<Enviroment>) -> Self {
0201:         let mut build = self;
0202:         build.env = Some(env.into());
0203:         build
0204:     }
0205: 
0206:     pub fn pdk(self, pdk: Arc<Pdk>) -> Self {
0207:         let mut build = self;
0208:         build.pdk = Some(pdk);
0209:         build
0210:     }
0211: 
0212:     pub fn policy<T: FunctionCharzPolicy + 'static>(mut self, policy: impl Into<Box<T>>) -> Self {
0213:         let policy: Box<T> = policy.into();
0214:         self.policy = Some(policy);
0215:         self
0216:     }
0217: 
0218:     pub fn command<T: SpiceCommand + 'static>(mut self, command: impl Into<Box<T>>) -> Self {
0219:         let command: Box<T> = command.into();
0220:         self.command = Some(command);
0221:         self
0222:     }
0223: 
0224:     pub fn policy_box(mut self, policy: Box<dyn FunctionCharzPolicy>) -> Self {
0225:         self.policy = Some(policy.into());
0226:         self
0227:     }
0228: 
0229:     pub fn command_box(mut self, command: Box<dyn SpiceCommand>) -> Self {
0230:         self.command = Some(command.into());
0231:         self
0232:     }
0233: 
0234:     pub fn temp_folder(self, temp_folder: impl Into<PathBuf>) -> Self {
0235:         let mut build = self;
0236:         build.temp_folder = Some(temp_folder.into());
0237:         build
0238:     }
0239: 
0240:     pub fn simulate_path(self, simulate_path: impl Into<PathBuf>) -> Self {
0241:         let mut build = self;
0242:         build.simulate_path = Some(simulate_path.into());
0243:         build
0244:     }
0245: 
0246:     pub fn circuit_path(self, circuit_path: impl Into<PathBuf>) -> Self {
0247:         let mut build = self;
0248:         build.circuit_path = Some(circuit_path.into());
0249:         build
0250:     }
0251: }

// File: YouRAM-master\src\charz\function\random.rs

0001: use std::collections::HashSet;
0002: use crate::YouRAMResult;
0003: use super::{FunctionCharzPolicy, FunctionTransactionGenerator};
0004: use rand::Rng;
0005: use tracing::debug;
0006: 
0007: pub struct RandomPolicy;
0008: 
0009: impl FunctionCharzPolicy for RandomPolicy {
0010:     /*
0011: 
0012:         1. 鐢熸垚 N 涓湴鍧€
0013: 
0014:         2. 鍏堢粰 N 涓湴鍧€鍐欏叆鍒濆鍊硷紝骞朵笖鍦ㄥ啓鍏ヤ竴涓湴鍧€鍚庨┈涓婃彃鍏ュ搴旂殑璇绘搷浣滐紒
0015: 
0016:         3. 寮€濮嬮殢鏈轰骇鐢熻/鍐欐搷浣滐紝浣嗚淇″彿鍦板潃蹇呴』鏉ヨ嚜杩?N 涓湴鍧€锛屽啓淇″彿鍙互涓嶉渶瑕?
0017:         鍦ㄤ骇鐢熻嚦灏?2*N 涓鎿嶄綔鍚庣粨鏉熴€?
0018:     */
0019:     fn generate_transactions(&self, charz: &mut FunctionTransactionGenerator) -> YouRAMResult<()> {
0020:         debug!("generate transactions with random policy");
0021:         let read_transaction_size = 1.max(( 0.1 * charz.transactions.sram.read().word_size() as f64 ) as usize);
0022:         let addresses = self.generate_random_address(charz, read_transaction_size)?;
0023: 
0024:         // 鍏堢粰 N 涓湴鍧€鍐欏叆鍒濆鍊硷紝骞朵笖鍦ㄥ啓鍏ヤ竴涓湴鍧€鍚庨┈涓婃彃鍏ュ搴旂殑璇绘搷浣滐紒
0025:         for &address in addresses.iter() {
0026:             charz.add_write_transaction(address, charz.transactions.random_word());
0027:             charz.add_read_transaction(address);
0028:         }
0029: 
0030:         // 寮€濮嬮殢鏈轰骇鐢熻/鍐欐搷浣滐紝浣嗚淇″彿鍦板潃蹇呴』鏉ヨ嚜杩?N 涓湴鍧€锛屽啓淇″彿鍙互涓嶉渶瑕?        let mut rng = rand::rng();
0031:         let target_size = 2usize.pow(read_transaction_size as u32);
0032:         while charz.transactions.read_transaction_size() <= target_size {
0033:             let is_write: bool = rng.random_bool(0.5);
0034:             if is_write {
0035:                 let address = charz.transactions.random_address();
0036:                 let word = charz.transactions.random_word();
0037:                 charz.add_write_transaction(address, word);
0038:             } else {
0039:                 let address_index = rng.random_range(0..addresses.len());
0040:                 assert!(address_index < addresses.len());
0041:                 let address = addresses[address_index];
0042:                 charz.add_read_transaction(address);
0043:             }
0044:         }
0045: 
0046:         Ok(())
0047:     }
0048: }
0049: 
0050: impl RandomPolicy {
0051:     fn generate_random_address(&self, charz: &mut FunctionTransactionGenerator, read_transaction_size: usize) -> YouRAMResult<Vec<usize>> {
0052:         let mut address_set = HashSet::new();
0053:         while address_set.len() < read_transaction_size {
0054:             let address = charz.transactions.random_address();
0055:             address_set.insert(address);
0056:         }
0057: 
0058:         Ok(address_set.into_iter().collect())
0059:     }
0060: }

// File: YouRAM-master\src\charz\timing\mod.rs

0001: use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, sync::Arc};
0002: use reda_unit::{Capacitance, Number, Time};
0003: use tracing::{debug, info};
0004: use crate::{circuit::{Shr, Sram}, export, pdk::{Enviroment, Pdk, Pvt}, simulate::{DelayMeasBuilder, Edge, NgSpice, SpiceCommand}, ErrorContext, YouRAMResult};
0005: use super::{CharzError, SramTransactionGenerator};
0006: 
0007: #[derive(Debug)]
0008: pub struct TimingCharzResult {
0009:     pub delay_hl: Time,
0010:     pub delay_lh: Time,
0011:     pub slew_hl: Time,
0012:     pub slew_lh: Time,
0013: }
0014: 
0015: /// Timeing charz for Sram
0016: /// 
0017: /// # Deafult:
0018: /// - command: ngspice
0019: /// - temp_folder: "./temp"
0020: /// - simulate_path: "./temp/simulator.sp"
0021: /// - circuit_path: "./temp/<sram_name>.sp"
0022: /// 
0023: /// # Example
0024: /// 
0025: /// ```no_run
0026: /// let result = TimingCharz::config()
0027: ///     .sram(sram)
0028: ///     .command(NgSpice)
0029: ///     .pdk(pdk)
0030: ///     .....
0031: ///     .period(t!(10 n))
0032: ///     .analyze()?;
0033: /// ```
0034: pub struct TimingCharz<'a> {
0035:     pub sram: Option<Shr<Sram>>,
0036:     pub period: Option<Time>,
0037:     pub pvt: Option<Pvt>,
0038:     pub input_net_transitions: Option<&'a [Time]>,
0039:     pub output_net_capacitances: Option<&'a [Capacitance]>,
0040:     pub pdk: Option<Arc<Pdk>>,
0041:     
0042:     pub command: Option<Box<dyn SpiceCommand>>,
0043: 
0044:     pub temp_folder: Option<PathBuf>,
0045:     pub simulate_path: Option<PathBuf>,
0046: 
0047:     /// if no circuit_path, create <temp>/<sram_name>.sp. if have, include it directiontly
0048:     pub circuit_path: Option<PathBuf>,
0049: }
0050: 
0051: impl<'a> TimingCharz<'a> {
0052:     pub fn analyze(self) -> YouRAMResult<Vec<Vec<TimingCharzResult>>> {
0053:         info!("execute timing charz");
0054:         
0055:         // extract all args
0056:         debug!("extract arguments");
0057:         let sram = self.sram.ok_or(CharzError::LackFunctionTestConfigField("sram"))?;
0058:         let period = self.period.ok_or(CharzError::LackFunctionTestConfigField("period"))?;
0059:         let pvt = self.pvt.ok_or(CharzError::LackFunctionTestConfigField("pvt"))?;
0060:         let pdk = self.pdk.ok_or(CharzError::LackFunctionTestConfigField("pdk"))?;
0061:         let input_net_transitions = self.input_net_transitions.ok_or(CharzError::LackFunctionTestConfigField("input_net_transitions"))?;
0062:         let output_net_capacitances = self.output_net_capacitances.ok_or(CharzError::LackFunctionTestConfigField("input_net_transitions"))?;
0063: 
0064:         let command = self.command.ok_or(CharzError::LackFunctionTestConfigField("command"))?;
0065: 
0066:         let temp_folder =  self.temp_folder.unwrap_or_else(|| "./temp".into());
0067:         let simulate_path = self.simulate_path.unwrap_or_else(|| temp_folder.join("simulate.sp"));
0068:         let circuit_path = match self.circuit_path {
0069:             Some(circuit_path) => circuit_path,
0070:             None => {
0071:                 let circuit_path = temp_folder.join(sram.read().name.to_string());
0072:                 export::write_spice(sram.clone(), &circuit_path).with_context(|| format!("write sram"))?;
0073:                 circuit_path
0074:             }
0075:         };
0076: 
0077:         // for all input_net_transition and output_net_capacitance
0078:         let mut all_result = vec![];
0079:         for &input_net_transition in input_net_transitions.iter() {
0080:             let mut result_in_same_slew = vec![];
0081:             for &output_net_capacitance in output_net_capacitances.iter() {
0082:                 let env = Enviroment::new(pvt.clone(), input_net_transition, output_net_capacitance);
0083:                 let result = Self::analyze_in_env(
0084:                     sram.clone(), 
0085:                     period, 
0086:                     env, 
0087:                     pdk.clone(), 
0088:                     &command, 
0089:                     &simulate_path, 
0090:                     &circuit_path, 
0091:                     &temp_folder
0092:                 )?;
0093:                 result_in_same_slew.push(result);
0094:             }
0095:             all_result.push(result_in_same_slew);
0096:         }
0097: 
0098:         Ok(all_result)
0099:     }
0100: 
0101:     fn analyze_in_env(
0102:         sram: Shr<Sram>, 
0103:         period: Time, 
0104:         env: Enviroment, 
0105:         pdk: Arc<Pdk>,
0106:         command: &impl SpiceCommand,
0107:         simulate_path: impl Into<PathBuf>,
0108:         circuit_path: impl Into<PathBuf>,
0109:         temp_folder: impl AsRef<Path>,
0110:     ) -> YouRAMResult<TimingCharzResult> {
0111:         let mut transactions = SramTransactionGenerator::new(sram, period);
0112:             
0113:         // generate some unique address
0114:         debug!("generate transactions");
0115:         let addresses: HashSet<usize> = Self::generate_random_address(&mut transactions);
0116: 
0117:         // for each address, write 0 + read 0, and add delay and slew meas
0118:         for &address in addresses.iter() {
0119:             transactions.add_write_transaction(address, 0);
0120:             transactions.add_read_transaction(address);
0121:         
0122:             let word_width = transactions.sram.read().word_width();
0123: 
0124:             /*
0125:                         +---+   +---+   +---+
0126:                 clk:    |   |   |   |   |   |
0127:                     +---+   +---+   +---+   +---
0128:                         |       |       |
0129:                         | write | read  | write | read  ...
0130:                 out:     |xxxxxxx|xxxxx--|
0131:                             |    ^
0132:                             |    |
0133:                             |    | get output
0134:                             |    |
0135:                             delay_hl
0136:             */  
0137: 
0138:             // read transaction's rise clock 
0139:             let time_delay = transactions.last_clock_rise_time() - transactions.half_period();
0140:             
0141:             // for all output bit, and meas
0142:             for bit in 0..word_width {
0143:                 let output_pin = Sram::data_output_pn(bit).to_string();
0144:                 
0145:                 // meas the clk rise to output down
0146:                 let meas = DelayMeasBuilder::default()
0147:                     .name(format!("delay_hl_d{}_b{}", address, bit))
0148:                     
0149:                     .trig_net_name(Sram::clock_pn().to_string())
0150:                     .trig_edge(Edge::Rise)
0151:                     .trig_voltage(env.voltage() * pdk.input_threshold_pct_rise())
0152:                     .trig_time_delay(time_delay)
0153:                     
0154:                     .targ_net_name(output_pin.clone())
0155:                     .targ_edge(Edge::Fall)
0156:                     .targ_voltage(env.voltage() * pdk.output_threshold_pct_fall())
0157:                     .targ_time_delay(time_delay)
0158:                     .build().unwrap();
0159:                 transactions.add_measurement(meas);
0160:     
0161:                 // meas from output to output
0162:                 let meas = DelayMeasBuilder::default()
0163:                     .name(format!("slew_hl_d{}_b{}", address, bit))
0164:                     
0165:                     .trig_net_name(output_pin.clone())
0166:                     .trig_edge(Edge::Fall)
0167:                     .trig_voltage(pdk.slew_upper_threshold_pct_fall() * env.voltage())
0168:                     .trig_time_delay(time_delay)
0169:                     
0170:                     .targ_net_name(output_pin.clone())
0171:                     .targ_edge(Edge::Fall)
0172:                     .targ_voltage(pdk.slew_lower_threshold_pct_fall() * env.voltage())
0173:                     .targ_time_delay(time_delay)
0174:                     .build().unwrap();
0175:                 transactions.add_measurement(meas);
0176:             }
0177:         }
0178: 
0179:         // simulate
0180:         debug!("spice simulate");
0181:         let result = transactions.simulate(env, pdk, command, simulate_path, Some(circuit_path), temp_folder)?;
0182: 
0183:         // extract result
0184:         debug!("extract timing result");
0185:         Self::extract_result(&result)
0186:     }
0187: 
0188:     fn generate_random_address(transactions: &mut SramTransactionGenerator) -> HashSet<usize> {
0189:         let total_address_size: usize = 2usize.pow(transactions.sram.read().address_width() as u32);
0190:         let address_count = 2.max(total_address_size / 10);
0191:         (0..address_count).map(|_| transactions.random_address()).collect()
0192:     }
0193: 
0194:     fn average(values: &[Number]) -> Number {
0195:         let mut sum = Number::zero();
0196:         let size = values.len();
0197:         for &v in values.iter() {
0198:             sum = sum + v;
0199:         }
0200:         sum / size as f64
0201:     }
0202: 
0203:     fn extract_result(result: &HashMap<String, Number>) -> YouRAMResult<TimingCharzResult> {
0204:         let mut delay_hls = vec![];
0205:         let mut slew_hls = vec![];
0206: 
0207:         for (name, value) in result.iter() {
0208:             if name.contains("delay_hl") {
0209:                 delay_hls.push(value.clone());
0210:             } else if name.contains("slew_hl") {
0211:                 slew_hls.push(value.clone());
0212:             }
0213:         }
0214: 
0215:         // average 
0216:         let delay_hl = Self::average(&delay_hls);
0217:         let slew_hl = Self::average(&slew_hls);
0218: 
0219:         Ok(TimingCharzResult { 
0220:             delay_hl: Time::from(delay_hl), 
0221:             delay_lh: Time::from(delay_hl), 
0222:             slew_hl: Time::from(slew_hl), 
0223:             slew_lh: Time::from(slew_hl) 
0224:         })
0225:     }
0226: }
0227: 
0228: impl<'a> Default for TimingCharz<'a> {
0229:     fn default() -> Self {
0230:         Self {
0231:             sram: None,
0232:             period: None,
0233:             pdk: None,
0234:             pvt: None,
0235:             input_net_transitions: None,
0236:             output_net_capacitances: None,
0237:             command: Some(Box::new(NgSpice)),
0238:             temp_folder: Some("./temp".into()),
0239:             simulate_path: None, 
0240:             circuit_path: None,
0241:         }
0242:     }
0243: }
0244: 
0245: impl<'a> TimingCharz<'a> {
0246:     pub fn config() -> Self {
0247:         Self::default()
0248:     }
0249: 
0250:     pub fn sram(self, sram: impl Into<Shr<Sram>>) -> Self {
0251:         let mut build = self;
0252:         build.sram = Some(sram.into());
0253:         build
0254:     }
0255: 
0256:     pub fn period(self, period: impl Into<Time>) -> Self {
0257:         let mut build = self;
0258:         build.period = Some(period.into());
0259:         build
0260:     }
0261: 
0262:     pub fn pvt(self, pvt: impl Into<Pvt>) -> Self {
0263:         let mut build = self;
0264:         build.pvt = Some(pvt.into());
0265:         build
0266:     }
0267: 
0268:     pub fn input_net_transitions(self, input_net_transitions: &'a [Time]) -> Self {
0269:         let mut build = self;
0270:         build.input_net_transitions = Some(input_net_transitions);
0271:         build
0272:     }
0273: 
0274:     pub fn output_net_capacitances(self, output_net_capacitances: &'a [Capacitance]) -> Self {
0275:         let mut build = self;
0276:         build.output_net_capacitances = Some(output_net_capacitances);
0277:         build
0278:     }
0279: 
0280:     pub fn pdk(self, pdk: Arc<Pdk>) -> Self {
0281:         let mut build = self;
0282:         build.pdk = Some(pdk);
0283:         build
0284:     }
0285: 
0286:     pub fn command<T: SpiceCommand + 'static>(mut self, command: impl Into<Box<T>>) -> Self {
0287:         let command: Box<T> = command.into();
0288:         self.command = Some(command);
0289:         self
0290:     }
0291: 
0292:     pub fn command_box(mut self, command: Box<dyn SpiceCommand>) -> Self {
0293:         self.command = Some(command.into());
0294:         self
0295:     }
0296: 
0297:     pub fn temp_folder(self, temp_folder: impl Into<PathBuf>) -> Self {
0298:         let mut build = self;
0299:         build.temp_folder = Some(temp_folder.into());
0300:         build
0301:     }
0302: 
0303:     pub fn simulate_path(self, simulate_path: impl Into<PathBuf>) -> Self {
0304:         let mut build = self;
0305:         build.simulate_path = Some(simulate_path.into());
0306:         build
0307:     }
0308: 
0309:     pub fn circuit_path(self, circuit_path: impl Into<PathBuf>) -> Self {
0310:         let mut build = self;
0311:         build.circuit_path = Some(circuit_path.into());
0312:         build
0313:     }
0314: }

// File: YouRAM-master\src\circuit\error.rs

0001: use super::{DriveStrength, LogicGateKind};
0002: 
0003: #[derive(Debug, thiserror::Error)]
0004: pub enum CircuitError {
0005:     #[error("port '{0}' already exit")]
0006:     AddDuplicatePort(String),
0007: 
0008:     #[error("instance '{0}' already exit")]
0009:     AddDuplicateInstance(String),
0010: 
0011:     #[error("unmatch pin size '{0}' and net size '{1}'")]
0012:     PinSizeUnmatch(usize, usize),
0013: 
0014:     #[error("expect '{0}' input but got '{1}'")]
0015:     LogicGateInputPinSizeUnmatch(usize, usize),
0016: 
0017:     #[error("instance '{0}' have not been connected")]
0018:     InstanceNotConnected(String),
0019: 
0020:     #[error("circuit arguments invalid: {0}")]
0021:     InvalidArguments(String),
0022: 
0023:     #[error("no exit port {0} in circuit {1}")]
0024:     PortNotFound(String, String),
0025: 
0026:     #[error("no exit pin {0} in instance {1}")]
0027:     PinNotFound(String, String),
0028: 
0029:     #[error("no exit instance {0} in module {1}")]
0030:     InstanceNotFound(String, String),
0031: 
0032:     #[error("no exit logicgate with({0}, {1})")]
0033:     LogicGateNotFound(LogicGateKind, DriveStrength),
0034: 
0035:     #[error("no exit dff with({0})")]
0036:     DffNotFound(DriveStrength),
0037: 
0038:     #[error("logicgate input order {0} port out of range ")]
0039:     LogicGateInputPortOutOfRange(usize),
0040: 
0041:     #[error("{0}")]
0042:     Messgae(String),
0043: }
0044: 
0045: impl CircuitError {
0046:     pub fn invalid_arg<S: Into<String>>(msg: S) -> Self {
0047:         Self::InvalidArguments(msg.into())
0048:     }
0049: 
0050:     pub fn msg<S: Into<String>>(msg: S) -> Self {
0051:         Self::Messgae(msg.into())
0052:     }
0053: }
0054: 
0055: #[macro_export]
0056: macro_rules! invalid_arg {
0057:     ($msg:literal $(,)?) => {
0058:         Err($crate::circuit::CircuitError::InvalidArguments(format!($msg).into()))?
0059:     };
0060:     ($err:expr $(,)?) => {
0061:         Err($crate::circuit::CircuitError::InvalidArguments(format!($err).into()))?
0062:     };
0063:     ($fmt:expr, $($arg:tt)*) => {
0064:         Err($crate::circuit::CircuitError::InvalidArguments(format!($fmt, $($arg)*).into()))?
0065:     };
0066: }
0067: 
0068: #[macro_export]
0069: macro_rules! check_arg {
0070:     ($cond:expr, $msg:literal $(,)?) => {
0071:         if !$cond {
0072:             Err($crate::circuit::CircuitError::InvalidArguments(format!($msg).into()))?
0073:         }
0074:     };
0075:     ($cond:expr, $msg:literal $(,)?) => {
0076:         if !$cond {
0077:             Err($crate::circuit::CircuitError::InvalidArguments(format!($err).into()))?
0078:         }
0079:     };
0080:     ($cond:expr, $fmt:expr, $($arg:tt)*) => {
0081:         if !$cond {
0082:             Err($crate::circuit::CircuitError::InvalidArguments(format!($fmt, $($arg)*).into()))?
0083:         }
0084:     }
0085: }

// File: YouRAM-master\src\circuit\factory.rs

0001: use std::any::{Any, TypeId};
0002: use std::{collections::HashMap, fmt::Debug};
0003: use std::sync::{Arc, RwLock};
0004: use tracing::info;
0005: use crate::pdk::Pdk;
0006: use crate::{ErrorContext, YouRAMResult};
0007: use super::{CircuitError, Dff, DriveStrength, Leafcell, LogicGate, LogicGateKind, Module, Shr, ShrString};
0008: 
0009: pub trait ModuleArg: Sized + Debug + Send + Sync {
0010:     fn module_name(&self) -> ShrString;
0011:     fn create_module(self, factory: &mut CircuitFactory) -> YouRAMResult<Module<Self>>;
0012: }
0013: 
0014: pub struct CircuitFactory {
0015:     pub pdk: Arc<Pdk>,
0016:     modules: HashMap<TypeId, HashMap<ShrString, Arc<dyn Any + Send + Sync>>>,
0017: }
0018: 
0019: impl CircuitFactory {
0020:     pub fn new(pdk: Arc<Pdk>) -> Self {
0021:         Self {
0022:             pdk,
0023:             modules: HashMap::new(),
0024:         }
0025:     }
0026: 
0027:     pub fn module<A: ModuleArg + 'static>(&mut self, arg: A) -> YouRAMResult<Shr<Module<A>>> {
0028:         let key = TypeId::of::<Module<A>>();
0029:         let entry = self.modules.entry(key);
0030:         let modules = entry.or_default();
0031:         
0032:         let name = arg.module_name();
0033:         match modules.get(&name) {
0034:             Some(module) => {
0035:                 let inner = module.clone().downcast_arc::<RwLock<Module<A>>>().unwrap();
0036:                 Ok(Shr::from_inner(inner))
0037:             }
0038:             None => {
0039:                 info!("create circuit '{}'", name);
0040:                 let module = arg.create_module(self).context(format!("create circuit '{}'", name))?;
0041:                 let module = Arc::new(RwLock::new(module));
0042:                 let entry = self.modules.entry(key);
0043:                 let modules = entry.or_default();
0044:                 modules.insert(name, module.clone());
0045:                 Ok(Shr::from_inner(module))
0046:             }
0047:         } 
0048:     }
0049: 
0050:     pub fn logicgate(&self, kind: LogicGateKind, drive_strength: DriveStrength) -> Result<Shr<LogicGate>, CircuitError> {
0051:         self.pdk.get_logicgate(kind, drive_strength)
0052:             .ok_or_else(|| CircuitError::LogicGateNotFound(kind, drive_strength))
0053:     }
0054: 
0055:     pub fn and(&self, input_size: usize, drive_strength: DriveStrength) -> Result<Shr<LogicGate>, CircuitError> {
0056:         self.pdk.get_and(input_size, drive_strength)
0057:             .ok_or_else(|| CircuitError::LogicGateNotFound(LogicGateKind::And(input_size), drive_strength))
0058:     }
0059: 
0060:     pub fn nand(&self, input_size: usize, drive_strength: DriveStrength) -> Result<Shr<LogicGate>, CircuitError> {
0061:         self.pdk.get_nand(input_size, drive_strength)
0062:             .ok_or_else(|| CircuitError::LogicGateNotFound(LogicGateKind::Nand(input_size), drive_strength))
0063:     }
0064: 
0065:     pub fn or(&self, input_size: usize, drive_strength: DriveStrength) -> Result<Shr<LogicGate>, CircuitError> {
0066:         self.pdk.get_or(input_size, drive_strength)
0067:             .ok_or_else(|| CircuitError::LogicGateNotFound(LogicGateKind::Or(input_size), drive_strength))
0068:     }
0069: 
0070:     pub fn nor(&self, input_size: usize, drive_strength: DriveStrength) -> Result<Shr<LogicGate>, CircuitError> {
0071:         self.pdk.get_or(input_size, drive_strength)
0072:             .ok_or_else(|| CircuitError::LogicGateNotFound(LogicGateKind::Nor(input_size), drive_strength))
0073:     }
0074: 
0075:     pub fn inv(&self, drive_strength: DriveStrength) -> Result<Shr<LogicGate>, CircuitError> {
0076:         self.pdk.get_inv(drive_strength)
0077:             .ok_or_else(|| CircuitError::LogicGateNotFound(LogicGateKind::Inv, drive_strength))
0078:     }
0079: 
0080:     pub fn dff(&self, drive_strength: DriveStrength) -> Result<Shr<Dff>, CircuitError> {
0081:         self.pdk.get_dff(drive_strength)
0082:             .ok_or_else(|| CircuitError::DffNotFound(drive_strength))
0083:     }
0084: 
0085:     pub fn bitcell(&self) -> Shr<Leafcell> {
0086:         self.pdk.get_bitcell()
0087:     }
0088: 
0089:     pub fn sense_amp(&self) -> Shr<Leafcell> {
0090:         self.pdk.get_sense_amp()
0091:     }
0092: 
0093:     pub fn column_trigate(&self) -> Shr<Leafcell> {
0094:         self.pdk.get_column_trigate()
0095:     }
0096: 
0097:     pub fn write_driver(&self) -> Shr<Leafcell> {
0098:         self.pdk.get_write_driver()
0099:     }
0100: 
0101:     pub fn precharge(&self) -> Shr<Leafcell> {
0102:         self.pdk.get_precharge()
0103:     }
0104: }
0105: 
0106: trait DowncastArc {
0107:     fn downcast_arc<T: Any + Send + Sync>(self: Arc<Self>) -> Result<Arc<T>, Arc<Self>>;
0108: }
0109: 
0110: impl DowncastArc for dyn Any + Send + Sync {
0111:     fn downcast_arc<T: Any + Send + Sync>(self: Arc<Self>) -> Result<Arc<T>, Arc<Self>> {
0112:         if self.is::<T>() {
0113:             let ptr = Arc::into_raw(self) as *const T;
0114:             Ok(unsafe { Arc::from_raw(ptr) })
0115:         } else {
0116:             Err(self)
0117:         }
0118:     }
0119: }

// File: YouRAM-master\src\circuit\mod.rs

0001: mod srdstring;
0002: mod shared;
0003: mod module;
0004: mod primitive;
0005: mod base;
0006: mod error;
0007: mod factory;
0008: 
0009: use std::sync::{MappedRwLockReadGuard, RwLockReadGuard};
0010: 
0011: pub use shared::*;
0012: pub use srdstring::ShrString;
0013: pub use module::*;
0014: pub use primitive::*;
0015: pub use base::*;
0016: pub use error::*;
0017: pub use factory::*;
0018: 
0019: pub trait Design {
0020:     fn name(&self) -> ShrString;
0021:     fn ports(&self) -> &[Shr<Port>];
0022:     fn get_port(&self, name: &str) -> Option<Shr<Port>> {
0023:         for port in self.ports() {
0024:             if port.read().name == name {
0025:                 return Some(port.clone());
0026:             }
0027:         }
0028:         None
0029:     }  
0030: }
0031: 
0032: #[derive(Clone, PartialEq, Eq, Hash)]
0033: pub enum ShrCircuit {
0034:     Module(Shr<dyn Modular>),
0035:     Primitive(Shr<dyn Primitive>),
0036: }
0037: 
0038: impl Into<ShrCircuit> for Shr<dyn Modular> {
0039:     fn into(self) -> ShrCircuit {
0040:         ShrCircuit::Module(self)
0041:     }
0042: }
0043: 
0044: impl Into<ShrCircuit> for Shr<LogicGate> {
0045:     fn into(self) -> ShrCircuit {
0046:         ShrCircuit::Primitive(self.into())
0047:     }
0048: }
0049: 
0050: impl Into<ShrCircuit> for Shr<Leafcell> {
0051:     fn into(self) -> ShrCircuit {
0052:         ShrCircuit::Primitive(self.into())
0053:     }
0054: }
0055: 
0056: impl Into<ShrCircuit> for Shr<Dff> {
0057:     fn into(self) -> ShrCircuit {
0058:         ShrCircuit::Primitive(self.into())
0059:     }
0060: }
0061: 
0062: impl<A: ModuleArg + 'static> Into<ShrCircuit> for Shr<Module<A>> {
0063:     fn into(self) -> ShrCircuit {
0064:         let module: Shr<dyn Modular> = self.into();
0065:         ShrCircuit::Module(module)
0066:     }
0067: }
0068: 
0069: impl ShrCircuit {
0070:     pub fn name(&self) -> ShrString {
0071:         match self {
0072:             Self::Module(module) => module.read().name(),
0073:             Self::Primitive(p) => p.read().name(),
0074:         }
0075:     }
0076:     
0077:     pub fn ports(&self) -> MappedRwLockReadGuard<'_, [Shr<Port>]> {
0078:         match self {
0079:             Self::Module(m) => RwLockReadGuard::map(m.read(), |m| m.ports()),
0080:             Self::Primitive(p) => RwLockReadGuard::map(p.read(), |p| p.ports()),
0081:         }
0082:     }
0083: 
0084:     pub fn port_names(&self) -> Vec<ShrString> {
0085:         self.ports().iter().map(|p| p.read().name.clone()).collect()
0086:     }
0087: 
0088:     pub fn is_moudle(&self) -> bool {
0089:         match self {
0090:             Self::Module(_) => true,
0091:             _ => false,
0092:         }
0093:     }
0094: 
0095:     pub fn is_primitive(&self) -> bool {
0096:         match self {
0097:             Self::Primitive(_) => true,
0098:             _ => false,
0099:         }
0100:     }
0101: 
0102:     pub fn moudle(&self) -> Option<Shr<dyn Modular>> {
0103:         match self {
0104:             Self::Module(module) => Some(module.clone()),
0105:             _ => None,
0106:         }
0107:     }
0108: 
0109:     pub fn primitive(&self) -> Option<Shr<dyn Primitive>> {
0110:         match self {
0111:             Self::Primitive(p) => Some(p.clone()),
0112:             _ => None,
0113:         }
0114:     }
0115: }

// File: YouRAM-master\src\circuit\module.rs

0001: macro_rules! register_module {
0002:     ($name:ident) => {
0003:         mod $name;
0004:         pub use $name::*;
0005:     };
0006: }
0007: register_module!(buffer);
0008: register_module!(decoder);
0009: register_module!(controllogic);
0010: register_module!(bitcellarray);
0011: register_module!(bitcellarrayrec);
0012: register_module!(writedriverarray);
0013: register_module!(wordlinederiverarr);
0014: register_module!(wordlinederiver);
0015: register_module!(senseamparray);
0016: register_module!(prechargearray);
0017: register_module!(fanoutbuffer);
0018: register_module!(datapath);
0019: register_module!(columnmuxarray);
0020: register_module!(columnmux);
0021: register_module!(bank);
0022: register_module!(andarray);
0023: register_module!(replicalbitcellarray);
0024: register_module!(core);
0025: register_module!(sram);
0026: register_module!(inputdffs);
0027: register_module!(coreselect);
0028: 
0029: use tracing::debug;
0030: 
0031: use std::{collections::{HashMap, HashSet}, mem::MaybeUninit, ops::Deref, sync::{Arc, RwLock}};
0032: use crate::{YouRAMResult, ErrorContext};
0033: use super::{CircuitError, CircuitFactory, Design, Dff, DriveStrength, Instance, LogicGate, LogicGateKind, ModuleArg, Net, Pin, Port, PortDirection, Shr, ShrCircuit, ShrString};
0034: 
0035: pub trait Modular: Design + Send + Sync {
0036:     fn instances(&self) -> &[Shr<Instance>];
0037:     fn sub_circuits(&self) -> &HashSet<ShrCircuit>;
0038:     fn connected_nets(&self) -> &[(Shr<Net>, Shr<Net>)];
0039: }
0040: 
0041: pub struct Module<A> {
0042:     pub name: ShrString,
0043:     pub ports: Vec<Shr<Port>>,
0044:     pub instances: Vec<Shr<Instance>>,
0045:     
0046:     pub sub_circuits: HashSet<ShrCircuit>,
0047: 
0048:     pub nets: HashMap<ShrString, Shr<Net>>,
0049:     pub connected_nets: Vec<(Shr<Net>, Shr<Net>)>,
0050: 
0051:     pub args: A,
0052: }
0053: 
0054: impl<A: ModuleArg + 'static> Into<Shr<dyn Modular>> for Shr<Module<A>> {
0055:     fn into(self) -> Shr<dyn Modular> {
0056:         let inner = self.inner();
0057:         let inner: Arc<RwLock<dyn Modular>> = inner;
0058:         Shr::from_inner(inner)
0059:     }
0060: }
0061: 
0062: pub trait AsInstance<A> : Clone {
0063:     fn as_instance(self, module: &Module<A>) -> Result<Shr<Instance>, CircuitError>;
0064: }
0065: 
0066: pub trait AsPin : Clone {
0067:     fn as_pin(self, instance: &Shr<Instance>) -> Result<Shr<Pin>, CircuitError>;
0068: }
0069: 
0070: macro_rules! impl_link_instance {
0071:     ($fn_name:ident, $factory_fn:ident, [$($port:ident),+]) => {
0072:         pub fn $fn_name(
0073:             &mut self,
0074:             factory: &mut CircuitFactory,
0075:             name: impl Into<ShrString>,
0076:             $($port: impl Into<ShrString>),+
0077:         ) -> YouRAMResult<Shr<Instance>> {
0078:             let name: ShrString = name.into();
0079:             (|| -> YouRAMResult<Shr<Instance>> {
0080:                 let cell = factory.$factory_fn();
0081:                 self.sub_circuits.insert(cell.clone().into());
0082:                 let instance = self.add_instance(name.clone(), cell)?;
0083:                 self.connect_instance(instance.clone(), [$($port.into()),+].into_iter())?;
0084:                 Ok(instance)
0085:             })()
0086:             .with_context(|| format!("link leafcell {} to circuit {}", name, self.name))
0087:         }
0088:     };
0089: }
0090: 
0091: impl<A> Module<A> {
0092:     pub fn new<S: Into<ShrString>>(name: S, args: A) -> Self {
0093:         Self {
0094:             name: name.into(), 
0095:             ports: Vec::new(),
0096:             instances: Vec::new(),
0097:             sub_circuits: HashSet::new(),
0098:             nets: HashMap::new(),
0099:             connected_nets: Vec::new(),
0100:             args
0101:         }
0102:     }
0103: 
0104:     pub fn add_port<S: Into<ShrString>>(&mut self, name: S, direction: PortDirection) -> YouRAMResult<Shr<Port>> {
0105:         let name: ShrString = name.into();
0106:         debug!("add port {} to circuit {}", name, self.name);
0107:         
0108:         (|| {
0109:             if self.ports.iter().any(|port| port.read().name == name) {
0110:                 Err(CircuitError::AddDuplicatePort(name.to_string()))
0111:             } else {
0112:                 // add a port and net with the same name
0113:                 let port = Port::new(name.clone(), direction);
0114:                 let net = self.add_net(port.read().name.clone());
0115:                 port.wrire().set_connected_net(net.clone());
0116:                 net.wrire().add_connection(port.clone());
0117:                 self.ports.push(port.clone());
0118:                 Ok(port)
0119:             }
0120:         })()
0121:         .with_context(|| format!("add port {} to circuit {}", name, self.name))
0122:     }
0123: 
0124:     pub fn add_module<Arg: ModuleArg + 'static>(&mut self, arg: Arg, factory: &mut CircuitFactory) -> YouRAMResult<Shr<Module<Arg>>> {   
0125:         (|| -> YouRAMResult<Shr<Module<Arg>>> {
0126:             debug!("add sub module to circuit {}", self.name);
0127:             let module = factory.module(arg)?;
0128:             self.sub_circuits.insert(module.clone().into());
0129:             Ok(module) 
0130:         })()  
0131:         .with_context(|| format!("add sub module to circuit {}", self.name))
0132:     }
0133: 
0134:     pub fn add_logicgate(&mut self, kind: LogicGateKind, drive_strength: DriveStrength, factory: &mut CircuitFactory) -> YouRAMResult<Shr<LogicGate>> {
0135:         (|| -> YouRAMResult<Shr<LogicGate>> {
0136:             debug!("add logicgate {}, {} to circuit {}", kind, drive_strength, self.name);
0137:             let logicgate = factory.logicgate(kind, drive_strength)?;
0138:             self.sub_circuits.insert(logicgate.clone().into());
0139:             Ok(logicgate)
0140:         })()  
0141:         .with_context(|| format!("add logicgate ({},{}) to circuit {}", kind, drive_strength, self.name))
0142:     }
0143: 
0144:     pub fn add_dff(&mut self, drive_strength: DriveStrength, factory: &mut CircuitFactory) -> YouRAMResult<Shr<Dff>> {
0145:         (|| -> YouRAMResult<Shr<Dff>> {
0146:             debug!("add dff {} to circuit {}", drive_strength, self.name);
0147:             let dff = factory.dff(drive_strength)?;
0148:             self.sub_circuits.insert(dff.clone().into());
0149:             Ok(dff)
0150:         })()  
0151:         .with_context(|| format!("add dff ({}) to circuit {}", drive_strength, self.name))
0152:     }
0153: 
0154:     pub fn add_instance<S, C>(&mut self, name: S, template_circuit: Shr<C>) -> YouRAMResult<Shr<Instance>> 
0155:     where 
0156:         S: Into<ShrString>,
0157:         C: Design,
0158:         Shr<C>: Into<ShrCircuit>,
0159:     {
0160:         let name: ShrString = name.into();
0161:         debug!("add instance {} to circuit {}", name, self.name);
0162: 
0163:         (|| {    
0164:             if self.instances.iter().any(|inst| inst.read().name == name) {
0165:                 return Err(CircuitError::AddDuplicateInstance(name.to_string()));
0166:             }
0167:     
0168:             // Create a new instance
0169:             let instance = Instance::new(name.clone(), template_circuit);
0170:             self.instances.push(instance.clone());
0171:             
0172:             Ok(instance)
0173:         })()
0174:         .with_context(|| format!("add instance {} to circuit {}", name, self.name))
0175:     }
0176: 
0177:     impl_link_instance!(link_bitcell_instance, bitcell, [bl, br, wl, vdd, gnd]);
0178:     impl_link_instance!(link_senseamp_instance, sense_amp, [bl, br, dout, en, vdd, gnd]);
0179:     impl_link_instance!(link_writedriver_instance, write_driver, [din, bl, br, en, vdd, gnd]);
0180:     impl_link_instance!(link_column_trigate_instance, column_trigate, [bl_in, br_in, bl_out, br_out, sel, vdd, gnd]);
0181:     impl_link_instance!(link_precharge_instance, precharge, [bl, br, en, vdd]);
0182: 
0183:     pub fn link_dff_instance(
0184:         &mut self, 
0185:         name: impl Into<ShrString>,
0186:         dff: Shr<Dff>,
0187:         din: impl Into<ShrString>,
0188:         clk: impl Into<ShrString>,
0189:         q: impl Into<ShrString>,
0190:         qn: impl Into<ShrString>,
0191:         vdd: impl Into<ShrString>,
0192:         gnd: impl Into<ShrString>,
0193:     ) -> YouRAMResult<Shr<Instance>> {
0194:         let name: ShrString = name.into();
0195:         (|| -> YouRAMResult<Shr<Instance>>  {
0196:             let dff_ref = dff.read();
0197:             
0198:             let instance = self.add_instance(name.clone(), dff.clone())?;
0199: 
0200:             let mut nets: Vec<MaybeUninit<ShrString>> = Vec::with_capacity(6);
0201:             for _ in 0..6 {
0202:                 nets.push(MaybeUninit::uninit());
0203:             }
0204:             unsafe { nets.get_unchecked_mut(dff_ref.din_port_index).write(din.into()); }
0205:             unsafe { nets.get_unchecked_mut(dff_ref.clk_port_index).write(clk.into()); }
0206:             unsafe { nets.get_unchecked_mut(dff_ref.q_port_index).write(q.into()); }
0207:             unsafe { nets.get_unchecked_mut(dff_ref.qn_port_index).write(qn.into()); }
0208:             unsafe { nets.get_unchecked_mut(dff_ref.vdd_port_index).write(vdd.into()); }
0209:             unsafe { nets.get_unchecked_mut(dff_ref.gnd_port_index).write(gnd.into()); }
0210:             
0211:             let nets = unsafe {
0212:                 std::mem::transmute::<Vec<MaybeUninit<ShrString>>, Vec<ShrString>>(nets)
0213:             };
0214:             
0215:             self.connect_instance(instance.clone(), nets.into_iter())?;
0216: 
0217:             Ok(instance)
0218:         })()
0219:         .with_context(|| format!("connect dff instance {} to circuit {}", name, self.name))
0220:     }
0221: 
0222:     pub fn link_inv_instance(
0223:         &mut self,
0224:         name: impl Into<ShrString>, 
0225:         logicgate: Shr<LogicGate>, 
0226:         nets: [impl Into<ShrString>; 4]
0227:     ) -> YouRAMResult<Shr<Instance>> {
0228:         let [input, output, vdd, gnd] = nets;
0229:         self.link_logicgate_instance(name, logicgate, vec![input], output, vdd, gnd)
0230:     }
0231: 
0232:     pub fn link_logicgate_instance(
0233:         &mut self, 
0234:         name: impl Into<ShrString>, 
0235:         logicgate: Shr<LogicGate>, 
0236:         input_nets: Vec<impl Into<ShrString>>, 
0237:         output_net: impl Into<ShrString>,
0238:         vdd_net: impl Into<ShrString>,
0239:         gnd_net: impl Into<ShrString>,
0240:     ) -> YouRAMResult<Shr<Instance>> {
0241:         let name: ShrString = name.into();
0242:         (|| -> YouRAMResult<Shr<Instance>>  {
0243:             let logicgate_ref = logicgate.read();
0244:             let expect_input_len = logicgate_ref.input_port_indexs.len();
0245:             let port_len = expect_input_len + 3;
0246: 
0247:             if expect_input_len != input_nets.len() {
0248:                 return Err(CircuitError::LogicGateInputPinSizeUnmatch(expect_input_len, input_nets.len()))?;
0249:             }
0250: 
0251:             let instance = self.add_instance(name.clone(), logicgate.clone())?;
0252: 
0253:             let mut nets: Vec<MaybeUninit<ShrString>> = Vec::with_capacity(port_len);
0254:             for _ in 0..port_len {
0255:                 nets.push(MaybeUninit::uninit());
0256:             }
0257: 
0258:             for (input_index, input_net) in input_nets.into_iter().enumerate() {
0259:                 let idx = logicgate_ref.input_port_indexs[input_index];
0260:                 unsafe { nets.get_unchecked_mut(idx).write(input_net.into()); }
0261:             }
0262:             
0263:             unsafe { nets.get_unchecked_mut(logicgate_ref.output_port_index).write(output_net.into()); }
0264:             unsafe { nets.get_unchecked_mut(logicgate_ref.vdd_port_index).write(vdd_net.into()); }
0265:             unsafe { nets.get_unchecked_mut(logicgate_ref.gnd_port_index).write(gnd_net.into()); }
0266:             
0267:             let nets = unsafe {
0268:                 std::mem::transmute::<Vec<MaybeUninit<ShrString>>, Vec<ShrString>>(nets)
0269:             };
0270:             
0271:             self.connect_instance(instance.clone(), nets.into_iter())?;
0272: 
0273:             Ok(instance)
0274:         })()
0275:         .with_context(|| format!("connect logicgate instance {} to circuit {}", name, self.name))
0276:     }
0277: 
0278:     pub fn link_module_instance<Arg, N, S, I>(&mut self, name: N, template_module: Shr<Module<Arg>>, nets: I) -> YouRAMResult<Shr<Instance>> 
0279:     where 
0280:         Arg: ModuleArg + 'static,
0281:         N: Into<ShrString>,
0282:         S: Into<ShrString>,
0283:         I: ExactSizeIterator<Item = S>,
0284:     {
0285:         let instance = self.add_instance(name, template_module)?;
0286:         self.connect_instance(instance.clone(), nets)?;
0287:         Ok(instance)
0288:     }
0289: 
0290:     pub fn connect_instance<'a, T, S, I>(&mut self, instance: T, nets: I) -> YouRAMResult<()> 
0291:     where 
0292:         T: AsInstance<A>,
0293:         S: Into<ShrString>,
0294:         I: ExactSizeIterator<Item = S>,
0295:     {        
0296:         let instance = instance.as_instance(self)?;
0297:         (|| -> YouRAMResult<()>  {
0298:             if instance.read().pins.len() != nets.len() {
0299:                 Err(CircuitError::PinSizeUnmatch(instance.read().pins.len(), nets.len()))?;
0300:             }
0301:             
0302:             debug!("connect instance {} to circuit {}", instance.read().name, self.name);
0303:             for (pin, net) in instance.read().pins.iter().zip(nets) {
0304:                 let net: ShrString = net.into();
0305:                 self.connect_pin_with_net(instance.clone(), pin, net)?;
0306:             }
0307:     
0308:             Ok(())
0309:         })()
0310:         .with_context(|| format!("connect instance {} to circuit {}", instance.read().name, self.name))
0311:     }
0312: 
0313:     pub fn connect_instance_with_map<'a, T, P, S, I>(&mut self, instance: T, pin_to_nets: I) -> YouRAMResult<()> 
0314:     where 
0315:         T: AsInstance<A>,
0316:         P: AsPin,
0317:         S: Into<ShrString>,
0318:         I: ExactSizeIterator<Item = (P, S)>,
0319:     {
0320:         let instance = instance.as_instance(self)?;
0321:         (|| -> YouRAMResult<()>  {
0322:             if instance.read().pins.len() != pin_to_nets.len() {
0323:                 Err(CircuitError::PinSizeUnmatch(instance.read().pins.len(), pin_to_nets.len()))?;
0324:             }
0325: 
0326:             debug!("connect instance {} to circuit {}", instance.read().name, self.name);
0327:             for (pin, net) in pin_to_nets {
0328:                 self.connect_pin_with_net(instance.clone(), pin, net)?;
0329:             }
0330:             Ok(())
0331:         })()
0332:         .with_context(|| format!("connect instance {} to circuit {}", instance.read().name, self.name))
0333:     }
0334: 
0335:     pub fn connect_pin_with_net(&mut self, instance: impl AsInstance<A>, pin: impl AsPin, net: impl Into<ShrString>) -> Result<Shr<Net>, CircuitError> {
0336:         let instance = instance.as_instance(self)?;
0337:         
0338:         let pin = pin.as_pin(&instance)?;
0339:         let net = self.add_net(net);
0340:         
0341:         debug!("connect pin {} with net {}", pin.read().name, net.read().name);
0342: 
0343:         pin.wrire().set_connected_net(net.clone());
0344:         net.wrire().add_connection(pin.clone());
0345: 
0346:         Ok(net)
0347:     }
0348: 
0349:     pub fn connect_nets(&mut self, net1: impl Into<ShrString>, net2: impl Into<ShrString>) {
0350:         let net1 = self.add_net(net1);
0351:         let net2 = self.add_net(net2);
0352:         self.connected_nets.push((net1, net2));
0353:     }
0354: 
0355:     pub fn add_net<S: Into<ShrString>>(&mut self, name: S) -> Shr<Net> {
0356:         let name_str = name.into();
0357:         if let Some(net) = self.nets.get(&name_str) {
0358:             net.clone()
0359:         } else {
0360:             let net = Net::new(name_str.clone());
0361:             self.nets.insert(name_str, net.clone());
0362:             net
0363:         }
0364:     }
0365: }
0366: 
0367: impl<A> AsInstance<A> for Shr<Instance> {
0368:     fn as_instance(self, _: &Module<A>) -> Result<Self, CircuitError> {
0369:         // Check instance
0370:         Ok(self)
0371:     }
0372: } 
0373: 
0374: impl<A> AsInstance<A> for &str {
0375:     fn as_instance(self, module: &Module<A>) -> Result<Shr<Instance>, CircuitError> {
0376:         module.instances.iter()
0377:             .find(|instance| instance.read().name == self)
0378:             .cloned()
0379:             .ok_or_else(|| CircuitError::InstanceNotFound(self.to_string(), module.name.to_string()))
0380:     }
0381: }
0382: 
0383: impl AsPin for &str {
0384:     fn as_pin(self, instance: &Shr<Instance>) -> Result<Shr<Pin>, CircuitError> {
0385:         instance.read()
0386:             .get_pin(self)
0387:             .ok_or_else(|| CircuitError::PinNotFound(self.to_string(), instance.read().name.to_string()))
0388:     }
0389: }
0390: 
0391: impl AsPin for Shr<Pin> {
0392:     fn as_pin(self, _: &Shr<Instance>) -> Result<Shr<Pin>, CircuitError> {
0393:         // check self
0394:         Ok(self)
0395:     }
0396: }
0397: 
0398: impl AsPin for &Shr<Pin> {
0399:     fn as_pin(self, _: &Shr<Instance>) -> Result<Shr<Pin>, CircuitError> {
0400:         // check self
0401:         Ok(self.clone())
0402:     }
0403: }
0404: 
0405: impl<A> Design for Module<A> {
0406:     fn name(&self) -> ShrString {
0407:         self.name.clone()
0408:     }
0409: 
0410:     fn ports(&self) -> &[Shr<Port>] {
0411:         &self.ports
0412:     }
0413: }
0414: 
0415: impl<A: Sync + Send> Modular for Module<A> {
0416:     fn sub_circuits(&self) -> &HashSet<ShrCircuit> {
0417:         &self.sub_circuits
0418:     }
0419: 
0420:     fn connected_nets(&self) -> &[(Shr<Net>, Shr<Net>)] {
0421:         &self.connected_nets
0422:     }
0423: 
0424:     fn instances(&self) -> &[Shr<Instance>] {
0425:         &self.instances
0426:     }
0427: }
0428: 
0429: impl Design for Box<dyn Modular> {
0430:     fn name(&self) -> ShrString {
0431:         self.deref().name()
0432:     }
0433: 
0434:     fn ports(&self) -> &[Shr<Port>] {
0435:         self.deref().ports()   
0436:     }
0437: }

// File: YouRAM-master\src\circuit\shared.rs

0001: 
0002: use std::{hash::{Hash, Hasher}, ptr, sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard}};
0003: 
0004: #[derive(Debug)]
0005: pub struct Shr<T: ?Sized> {
0006:     inner: Arc<RwLock<T>>,
0007: }
0008: 
0009: impl<T: ?Sized> Shr<T> {
0010:     pub fn from_inner(inner: Arc<RwLock<T>>) -> Self {
0011:         Self { inner }
0012:     }
0013: }
0014: 
0015: impl<T> Shr<T> {
0016:     pub fn new(value: T) -> Self {
0017:         Self {
0018:             inner: Arc::new(RwLock::new(value)),
0019:         }
0020:     }
0021: 
0022:     pub fn inner(self) -> Arc<RwLock<T>> {
0023:         self.inner
0024:     }
0025: }
0026: 
0027: impl<T: ?Sized> Shr<T> {
0028:     pub fn read(&self) -> RwLockReadGuard<'_, T> {
0029:         self.inner.read().unwrap()
0030:     }
0031: 
0032:     pub fn wrire(&self) -> RwLockWriteGuard<'_, T> {
0033:         self.inner.write().unwrap()
0034:     }
0035: }
0036: 
0037: impl<T: ?Sized> Clone for Shr<T> {
0038:     fn clone(&self) -> Self {
0039:         Shr {
0040:             inner: Arc::clone(&self.inner),
0041:         }
0042:     }
0043: }
0044: 
0045: impl<T: ?Sized> PartialEq for Shr<T> {
0046:     fn eq(&self, other: &Self) -> bool {
0047:         Arc::ptr_eq(&self.inner, &other.inner)
0048:     }
0049: }
0050: 
0051: impl<T: ?Sized> Eq for Shr<T> {}
0052: 
0053: impl<T: ?Sized> Hash for Shr<T> {
0054:     fn hash<H: Hasher>(&self, state: &mut H) {
0055:         ptr::hash(Arc::as_ptr(&self.inner), state);
0056:     }
0057: }

// File: YouRAM-master\src\circuit\srdstring.rs

0001: use std::{borrow::Borrow, collections::HashMap, hash::{Hash, Hasher}, sync::{Arc, LazyLock, RwLock}};
0002: 
0003: #[derive(Debug, Clone)]
0004: pub enum ShrString {
0005:     Static(&'static str),
0006:     Dynamic(Arc<String>),
0007: }
0008: 
0009: #[derive(Default)]
0010: pub struct StringPool {
0011:     map: HashMap<String, Arc<String>>,
0012: }
0013: 
0014: static POOL: LazyLock<RwLock<StringPool>> = LazyLock::new(|| {
0015:     RwLock::new(StringPool::default())
0016: });
0017: 
0018: impl ShrString {
0019:     pub fn new_str(s: &'static str) -> Self {
0020:         Self::Static(s)
0021:     }
0022: 
0023:     pub fn new_string<S: Into<String>>(s: S) -> Self {
0024:         let s = s.into();
0025:         let map = &mut POOL.write().unwrap().map;
0026: 
0027:         if let Some(existing) = map.get(&s) {
0028:             return ShrString::Dynamic(existing.clone());
0029:         }
0030: 
0031:         let arc = Arc::new(s);
0032:         map.insert(arc.to_string(), arc.clone());
0033:         Self::Dynamic(arc)
0034:     }
0035: 
0036:     pub fn as_str(&self) -> &str {
0037:         match self {
0038:             Self::Static(s) => s,
0039:             Self::Dynamic(s) => s.as_str()
0040:         }
0041:     }
0042: }
0043: 
0044: impl Default for ShrString {
0045:     fn default() -> Self {
0046:         Self::new_str("")
0047:     }
0048: }
0049: 
0050: impl std::ops::Deref for ShrString {
0051:     type Target = str;
0052:     fn deref(&self) -> &Self::Target {
0053:         self.as_str()
0054:     }
0055: }
0056: 
0057: impl std::fmt::Display for ShrString {
0058:     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
0059:         f.write_str(self.as_str())
0060:     }
0061: }
0062: 
0063: impl PartialEq for ShrString {
0064:     fn eq(&self, other: &Self) -> bool {
0065:         self.as_str() == other.as_str()
0066:     }
0067: }
0068: 
0069: impl Eq for ShrString {}
0070: 
0071: impl Hash for ShrString {
0072:     fn hash<H: Hasher>(&self, state: &mut H) {
0073:         self.as_str().hash(state)
0074:     }
0075: }
0076: 
0077: impl PartialEq<&str> for ShrString {
0078:     fn eq(&self, other: &&str) -> bool {
0079:         self.as_str() == *other
0080:     }
0081: }
0082: impl PartialEq<ShrString> for &str {
0083:     fn eq(&self, other: &ShrString) -> bool {
0084:         *self == other.as_str()
0085:     }
0086: }
0087: 
0088: impl PartialEq<String> for ShrString {
0089:     fn eq(&self, other: &String) -> bool {
0090:         self.as_str() == other.as_str()
0091:     }
0092: }
0093: impl PartialEq<ShrString> for String {
0094:     fn eq(&self, other: &ShrString) -> bool {
0095:         self.as_str() == other.as_str()
0096:     }
0097: }
0098: 
0099: impl From<&'static str> for ShrString {
0100:     fn from(s: &'static str) -> Self {
0101:         ShrString::new_str(s)
0102:     }
0103: }
0104: 
0105: impl From<String> for ShrString {
0106:     fn from(s: String) -> Self {
0107:         ShrString::new_string(s)
0108:     }
0109: }
0110: 
0111: #[macro_export]
0112: macro_rules! format_shr {
0113:     ($($arg:tt)*) => {{
0114:         $crate::circuit::ShrString::new_string(format!($($arg)*))
0115:     }};
0116: }
0117: 
0118: impl Borrow<str> for ShrString {
0119:     fn borrow(&self) -> &str {
0120:         self.as_str()
0121:     }
0122: }
0123: 
0124: impl AsRef<str> for ShrString {
0125:     fn as_ref(&self) -> &str {
0126:         self.as_str()
0127:     }
0128: }
0129: 
0130: #[cfg(test)]
0131: mod tests {
0132:     use super::*;
0133: 
0134:     #[test]
0135:     fn test_static_str() {
0136:         let a = ShrString::new_str("hello");
0137:         let b = ShrString::from("hello");
0138:         assert_eq!(a, b);
0139:         assert_eq!(a.as_str(), "hello");
0140:         assert!(matches!(a, ShrString::Static(_)));
0141:     }
0142: 
0143:     #[test]
0144:     fn test_dynamic_string_pooling() {
0145:         let s1 = ShrString::new_string("abc");
0146:         let s2 = ShrString::new_string("abc");
0147: 
0148:         if let (ShrString::Dynamic(a1), ShrString::Dynamic(a2)) = (&s1, &s2) {
0149:             assert!(Arc::ptr_eq(a1, a2), "Expected pooled Arc to be shared");
0150:         } else {
0151:             panic!("Expected dynamic ShrString");
0152:         }
0153: 
0154:         let s3 = ShrString::new_string("abcd");
0155:         if let (ShrString::Dynamic(a1), ShrString::Dynamic(a3)) = (&s1, &s3) {
0156:             assert!(!Arc::ptr_eq(a1, a3));
0157:         }
0158:     }
0159: 
0160:     #[test]
0161:     fn test_display_and_deref() {
0162:         let s = ShrString::new_string("xyz");
0163:         assert_eq!(s.to_string(), "xyz");
0164:         assert_eq!(&*s, "xyz");
0165:     }
0166: 
0167:     #[test]
0168:     fn test_partial_eq_variants() {
0169:         let s = ShrString::new_string("test");
0170:         assert_eq!(s, "test");
0171:         assert_eq!("test", s);
0172:         assert_eq!(s, "test".to_string());
0173:         assert_eq!("test".to_string(), s);
0174:     }
0175: 
0176:     #[test]
0177:     fn test_hash_and_eq() {
0178:         use std::collections::HashSet;
0179:         let s1 = ShrString::new_string("foo");
0180:         let s2 = ShrString::new_string("foo");
0181:         let s3 = ShrString::new_string("bar");
0182: 
0183:         let mut set = HashSet::new();
0184:         set.insert(s1.clone());
0185:         assert!(set.contains(&s2));
0186:         assert!(!set.contains(&s3));
0187:     }
0188: 
0189:     #[test]
0190:     fn test_pool_is_global() {
0191:         let before = POOL.read().unwrap().map.len();
0192:         let _ = ShrString::new_string("pooled_test");
0193:         let after = POOL.read().unwrap().map.len();
0194:         assert!(after >= before, "Pool size should not decrease");
0195:     }
0196: 
0197:     #[test]
0198:     fn test_borrow_trait() {
0199:         use std::collections::HashMap;
0200:         let mut map: HashMap<ShrString, i32> = HashMap::new();
0201:         map.insert("key1".into(), 42);
0202:         assert_eq!(map.get("key1"), Some(&42));
0203:     }
0204: }
0205: 

// File: YouRAM-master\src\circuit\base\instance.rs

0001: use crate::circuit::{CircuitError, Design, Pin, Shr, ShrCircuit, ShrString};
0002: use super::Net;
0003: 
0004: pub struct Instance {
0005:     pub name: ShrString,
0006:     pub template_circuit: ShrCircuit,
0007:     pub pins: Vec<Shr<Pin>>,
0008: }
0009: 
0010: impl Instance {
0011:     pub fn new<S, C>(name: S, template_circuit: Shr<C>) -> Shr<Instance> 
0012:     where 
0013:         S: Into<ShrString>,
0014:         C: Design,
0015:         Shr<C>: Into<ShrCircuit>,
0016:     {
0017:         let pins = template_circuit.read().ports().iter().map(|port| {
0018:             Pin::new(port.read().name.clone(), port.clone())
0019:         })
0020:         .collect();
0021: 
0022:         let template_circuit: ShrCircuit = template_circuit.into();
0023:         let name: ShrString = name.into();
0024:         
0025:         Shr::new ( Self { name: name.into(), template_circuit, pins } )
0026:     }
0027: 
0028:     pub fn get_pin(&self, name: &str) -> Option<Shr<Pin>> {
0029:         for pin in self.pins.iter() {
0030:             if pin.read().name == name {
0031:                 return Some(pin.clone());
0032:             }
0033:         }
0034:         None
0035:     }
0036: 
0037:     pub fn connect_nets(&mut self, nets: &[Shr<Net>]) -> Result<(), CircuitError> {
0038:         if self.pins.len() != nets.len() {
0039:             Err(CircuitError::PinSizeUnmatch(self.pins.len(), nets.len()))
0040:         } else {
0041:             for (pin, net) in self.pins.iter().zip(nets.iter()) {
0042:                 pin.wrire().net = Some(net.clone());
0043:             }
0044:             Ok(())
0045:         }
0046:     }
0047: 
0048:     pub fn connect_net(&mut self, pin_name: &str, net: Shr<Net>) {
0049:         for pin in self.pins.iter_mut() {
0050:             if pin.read().name == pin_name {
0051:                 pin.wrire().net = Some(net);
0052:                 break;
0053:             }
0054:         }
0055:     }
0056: }

// File: YouRAM-master\src\circuit\base\mod.rs

0001: mod port;
0002: mod pin;
0003: mod net;
0004: mod instance;
0005: pub use port::*;
0006: pub use pin::*;
0007: pub use net::*;
0008: pub use instance::*;

// File: YouRAM-master\src\circuit\base\net.rs

0001: use crate::circuit::{ShrString, Shr};
0002: use super::{Pin, Port};
0003: 
0004: pub struct Net {
0005:     pub name: ShrString,
0006:     pub connections: Vec<NetNode>
0007: }
0008: 
0009: pub enum NetNode {
0010:     Port(Shr<Port>),
0011:     Pin(Shr<Pin>),
0012: }
0013: 
0014: impl Into<NetNode> for Shr<Port> {
0015:     fn into(self) -> NetNode {
0016:         NetNode::Port(self)
0017:     }
0018: }
0019: 
0020: impl Into<NetNode> for Shr<Pin> {
0021:     fn into(self) -> NetNode {
0022:         NetNode::Pin(self)
0023:     }
0024: }
0025: 
0026: impl Net {
0027:     pub fn new<S: Into<ShrString>>(name: S) -> Shr<Self> {
0028:         Shr::new( Self { name: name.into(), connections: vec![] } )
0029:     }
0030: 
0031:     pub fn add_connection<N: Into<NetNode>>(&mut self, node: N) {
0032:         self.connections.push(node.into());
0033:     }
0034: 
0035:     pub fn connect(&mut self, node1: impl Into<NetNode>, node2: impl Into<NetNode>) {
0036:         self.add_connection(node1);
0037:         self.add_connection(node2);
0038:     }
0039: }

// File: YouRAM-master\src\circuit\base\pin.rs

0001: use crate::circuit::{ShrString, Shr};
0002: 
0003: use super::{Net, Port};
0004: 
0005: pub struct Pin {
0006:     pub name: ShrString,
0007:     pub net: Option<Shr<Net>>,
0008:     pub template_port: Shr<Port>,
0009: }
0010: 
0011: pub type RcPin = Shr<Pin>;
0012: 
0013: impl Pin {
0014:     pub fn new<S: Into<ShrString>>(name: S, template_port: Shr<Port>) -> Shr<Self> {
0015:         Shr::new(Self {
0016:             name: name.into(), net: None, template_port
0017:         })
0018:     }
0019: 
0020:     pub fn connected(&self) -> bool {
0021:         self.net.is_some()
0022:     }
0023: 
0024:     pub fn set_connected_net(&mut self, net: Shr<Net>) {
0025:         self.net = Some(net)
0026:     }
0027: }

// File: YouRAM-master\src\circuit\base\port.rs

0001: use crate::circuit::{Shr, ShrString};
0002: use super::Net;
0003: 
0004: #[derive(Debug, Clone, Copy, PartialEq, Eq)]
0005: pub enum PortDirection {
0006:     Input,
0007:     Output,
0008:     InOut,
0009:     Vdd, 
0010:     Gnd,
0011: }
0012: 
0013: pub struct Port {
0014:     pub name: ShrString,
0015:     pub direction: PortDirection,
0016:     pub net: Option<Shr<Net>>,
0017: }
0018: 
0019: impl Port {
0020:     pub fn new<S: Into<ShrString>>(name: S, direction: PortDirection) -> Shr<Self> {
0021:         Shr::new( Self { name: name.into(), direction, net: None } )
0022:     }
0023: 
0024:     pub fn is_input(&self) -> bool {
0025:         self.direction == PortDirection::Input
0026:     }
0027: 
0028:     pub fn is_output(&self) -> bool {
0029:         self.direction == PortDirection::Output
0030:     }
0031: 
0032:     pub fn is_source(&self) -> bool {
0033:         self.direction == PortDirection::Vdd || self.direction == PortDirection::Gnd
0034:     }
0035: 
0036:     pub fn is_vdd(&self) -> bool {
0037:         self.direction == PortDirection::Vdd
0038:     }
0039: 
0040:     pub fn is_gnd(&self) -> bool {
0041:         self.direction == PortDirection::Gnd
0042:     }
0043: 
0044:     pub fn connected(&self) -> bool {
0045:         self.net.is_some()
0046:     }
0047: 
0048:     pub fn set_connected_net(&mut self, net: Shr<Net>) {
0049:         self.net = Some(net)
0050:     }
0051: }

// File: YouRAM-master\src\circuit\module\andarray.rs

0001: use youram_macro::module;
0002: use crate::{circuit::{CircuitFactory, DriveStrength, LogicGateKind}, format_shr, YouRAMResult};
0003: 
0004: #[module(
0005:     input:  ("A{size}", Input),
0006:     enbale: ("en", Input),
0007:     output: ("Z{size}", Output),
0008:     vdd:    ("vdd", Vdd),
0009:     gnd:    ("gnd", Gnd),
0010: )]
0011: pub struct AndArray {
0012:     pub size: usize,
0013: }
0014: 
0015: impl AndArray {
0016:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0017:         let and = self.add_logicgate(LogicGateKind::And(2), DriveStrength::X2, factory)?;
0018:         for and_index in 0..self.args.size {
0019:             self.link_logicgate_instance(
0020:                 format_shr!("and{}", and_index), 
0021:                 and.clone(), 
0022:                 vec![Self::input_pn(and_index), Self::enbale_pn()], 
0023:                 Self::output_pn(and_index), 
0024:                 Self::vdd_pn(), 
0025:                 Self::gnd_pn(),
0026:             )?;
0027:         }
0028: 
0029:         Ok(())
0030:     }
0031: }

// File: YouRAM-master\src\circuit\module\bank.rs

0001: use youram_macro::module;
0002: use crate::{check_arg, circuit::{CircuitFactory, DataPathArg, ReplicaBitcellArrayArg, ShrString}, format_shr, YouRAMResult};
0003: use super::{BitcellArrayRecursiveArg, PrechargeArrayArg};
0004: 
0005: #[module(
0006:     wordline_enbale:      ("wl_en", Input),
0007:     precharge_enbale_bar: ("p_en_bar", Input),
0008:     sense_amp_enable:     ("sa_en", Output),
0009:     write_driver_enable:  ("we_en", Input),
0010: 
0011:     wordline:             ("wl{row_size}", Input),
0012:     col_select:           ("csel{column_sel_size}", Input, "column_sel_size > 1"),
0013: 
0014:     data_input:           ("din{word_width}", Input),
0015:     data_output:          ("dout{word_width}", Input),
0016: 
0017:     replical_bitline:     ("rbl", InOut),
0018: 
0019:     vdd:                  ("vdd", Vdd),
0020:     gnd:                  ("gnd", Gnd),
0021: )]
0022: pub struct Bank {
0023:     pub row_size: usize,
0024:     pub column_sel_size: usize,
0025:     pub word_width: usize,
0026: 
0027:     #[new(value = "column_sel_size * word_width")]
0028:     pub column_size: usize,
0029: }
0030: 
0031: 
0032: impl Bank {
0033:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0034:         check_arg!(self.args.row_size >= 1, "row size {} < 1", self.args.row_size);
0035:         check_arg!(self.args.column_size >= 1, "column size {} < 1", self.args.column_size);
0036:         
0037:         let replical_bitcell_array 
0038:             = self.add_module(ReplicaBitcellArrayArg::new(self.args.row_size), factory)?;
0039:         let bitcell_array 
0040:             = self.add_module(BitcellArrayRecursiveArg::new(self.args.row_size, self.args.column_size), factory)?;
0041: 
0042:         let data_path
0043:             = self.add_module(DataPathArg::new(self.args.word_width, self.args.column_sel_size), factory)?;
0044:         let precharge_array 
0045:             = self.add_module(PrechargeArrayArg::new(self.args.column_size), factory)?;
0046:         
0047:         let bl_nets: Vec<_> = (0..self.args.column_size).map(|i| format_shr!("bl{}", i)).collect();
0048:         let br_nets: Vec<_> = (0..self.args.column_size).map(|i| format_shr!("br{}", i)).collect();
0049: 
0050:         let rbr_net: ShrString = "rbr".into();
0051:    
0052:         // bitcell array
0053:         {
0054:             let mut nets = vec![];
0055:             nets.extend(bl_nets.iter().cloned());
0056:             nets.extend(br_nets.iter().cloned());
0057:             nets.extend((0..self.args.row_size).map(|i| Self::wordline_pn(i)));
0058:             nets.push(Self::vdd_pn());
0059:             nets.push(Self::gnd_pn());
0060: 
0061:             self.link_module_instance("bitcell_array", bitcell_array, nets.into_iter())?;
0062:         }
0063: 
0064:         // replical bitcell array
0065:         {
0066:             let mut nets = vec![];
0067:             nets.push(Self::replical_bitline_pn());
0068:             nets.push(rbr_net.clone());
0069:             nets.push(Self::wordline_enbale_pn());
0070:             nets.push(Self::vdd_pn());
0071:             nets.push(Self::gnd_pn());
0072: 
0073:             self.link_module_instance("replical_bitcell_array", replical_bitcell_array, nets.into_iter())?;   
0074:         }  
0075:    
0076:         // precharge array
0077:         {
0078:             let mut nets = vec![];
0079:             nets.extend(bl_nets.iter().cloned());
0080:             nets.extend(br_nets.iter().cloned());
0081:             nets.push(Self::precharge_enbale_bar_pn());
0082:             nets.push(Self::vdd_pn());
0083: 
0084:             self.link_module_instance("precharge_array", precharge_array, nets.into_iter())?;
0085:         }
0086: 
0087:         // precharge for rbl
0088:         self.link_precharge_instance(factory, "precharge_rbl", 
0089:             Self::replical_bitline_pn(), rbr_net.clone(), Self::precharge_enbale_bar_pn(), Self::vdd_pn())?;
0090: 
0091:         // datapath
0092:         {
0093:             let mut nets = vec![];
0094:             nets.push(Self::sense_amp_enable_pn());
0095:             nets.push(Self::write_driver_enable_pn());
0096:             nets.extend(bl_nets.iter().cloned());
0097:             nets.extend(br_nets.iter().cloned());
0098:             if self.has_column_address() {
0099:                 nets.extend((0..self.args.column_sel_size).map(|i| Self::col_select_pn(i)));
0100:             }
0101: 
0102:             nets.extend((0..self.args.word_width).map(|i| Self::data_input_pn(i)));
0103:             nets.extend((0..self.args.word_width).map(|i| Self::data_output_pn(i)));
0104: 
0105:             nets.push(Self::vdd_pn());
0106:             nets.push(Self::gnd_pn());
0107: 
0108:             self.link_module_instance("datapath", data_path, nets.into_iter())?;
0109:         }
0110: 
0111:         // write driver for rbl
0112:         self.link_writedriver_instance(
0113:             factory, 
0114:             "writedriver", 
0115:             Self::gnd_pn(), 
0116:             Self::replical_bitline_pn(),
0117:             rbr_net.clone(),
0118:             Self::write_driver_enable_pn(),
0119:             Self::vdd_pn(),
0120:             Self::gnd_pn(),
0121:         )?;
0122: 
0123:         Ok(())
0124:     }
0125: 
0126:     pub fn has_column_address(&self) -> bool {
0127:         self.args.column_sel_size > 1
0128:     }
0129: }

// File: YouRAM-master\src\circuit\module\bitcellarray.rs

0001: use youram_macro::module;
0002: use crate::{check_arg, circuit::CircuitFactory, YouRAMResult};
0003: 
0004: #[module(
0005:     bitline:      ("bl{column_size}", InOut),
0006:     bitline_bar:  ("br{column_size}", InOut),
0007:     wordline:     ("wl{row_size}", Input),
0008:     vdd:          ("vdd", Vdd),
0009:     gnd:          ("gnd", Gnd),
0010: )]
0011: pub struct BitcellArray {
0012:     pub row_size: usize,
0013:     pub column_size: usize,   
0014: }
0015: 
0016: impl BitcellArray {
0017: 
0018:     /*
0019:     
0020:          +----------------------------+
0021:      wln |                            |
0022:          |                            |
0023:                                       |
0024:          .                            |
0025:          .                            |
0026:          .                            |
0027:                                       |
0028:      wl2 |                            |
0029:          |                            |
0030:      wl1 |                            |
0031:          |                            |
0032:      wl0 |                            |
0033:          |                            |
0034:          +----------------------------+
0035:             bl0 bl1 bl2   ...     bln
0036:     
0037:     */     
0038:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0039:         check_arg!(self.args.row_size >= 1, "row size {} < 1", self.args.row_size);
0040:         check_arg!(self.args.column_size >= 1, "column size {} < 1", self.args.column_size);
0041: 
0042:         for row in 0..self.args.row_size {
0043:             for col in 0..self.args.column_size {
0044:                 self.link_bitcell_instance(
0045:                     factory, 
0046:                     format!("bitcell_{}_{}", row, col), 
0047:                     Self::bitline_pn(col), 
0048:                     Self::bitline_bar_pn(col),
0049:                     Self::wordline_pn(row), 
0050:                     Self::vdd_pn(),
0051:                     Self::gnd_pn(),
0052:                 )?;
0053:             }
0054:         }
0055: 
0056:         Ok(())
0057:     }
0058: }

// File: YouRAM-master\src\circuit\module\bitcellarrayrec.rs

0001: use youram_macro::module;
0002: use crate::{check_arg, circuit::CircuitFactory, format_shr, YouRAMResult};
0003: 
0004: #[module(
0005:     bitline:      ("bl{column_size}", InOut),
0006:     bitline_bar:  ("br{column_size}", InOut),
0007:     wordline:     ("wl{row_size}", Input),
0008:     vdd:          ("vdd", Vdd),
0009:     gnd:          ("gnd", Gnd),
0010: )]
0011: pub struct BitcellArrayRecursive {
0012:     pub row_size: usize,
0013:     pub column_size: usize,   
0014: }
0015: 
0016: impl BitcellArrayRecursive {
0017: 
0018:     /*
0019: 
0020:         Divide bitcellarray to subarray and some bitcell. For 
0021:         - row size => rowSize = rowSubSize * rowSubCount + rowRemainder
0022:         - col size => colSize = colSubSize * colSubCount + colRemainder
0023: 
0024:         So, there are four kinds of subarray:
0025:         - 1. rowSubSize * colSubSize (rowSubCount * colSubCount)
0026:         - 2. rowRemainder * colSubSize (colSubSize) 
0027:         - 3. rowSubSize * colRemainder (rowSubSize)
0028:         - 4. rowRemainder * colRemainder (1)
0029: 
0030:         +-----------+-----------+-----------+---------------+-----+
0031:         |           |           |           |               |     |
0032:         |     1     |     1     |     1     |    .......    |  3  |
0033:         |           |           |           |               |     |
0034:         +-----------+-----------+-----------+---------------+-----+
0035:         |           |           |           |               |     |
0036:         |     1     |     1     |     1     |    .......    |  3  |
0037:         |           |           |           |               |     |
0038:         +-----------+-----------+-----------+---------------+-----+
0039:         |           |           |           |               |     |
0040:         |     1     |     1     |     1     |    .......    |  3  |
0041:         |           |           |           |               |     |
0042:         +-----------+-----------+-----------+---------------+-----+
0043:         |           |           |           |   .           |     |
0044:         |     .     |     .     |     .     |     .         |  .  |
0045:         |     .     |     .     |     .     |       .       |  .  |
0046:         |     .     |     .     |     .     |         .     |  .  |
0047:         |           |           |           |           .   |     |
0048:         +-----------+-----------+-----------+---------------+-----+
0049:         |     2     |     2     |     2     |    .......    |  4  |
0050:         +-----------+-----------+-----------+---------------+-----+
0051: 
0052:     */ 
0053:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0054:         check_arg!(self.args.row_size >= 1, "row size {} < 1", self.args.row_size);
0055:         check_arg!(self.args.column_size >= 1, "column size {} < 1", self.args.column_size);
0056: 
0057:         if self.args.row_size >= 4 || self.args.column_size >= 4 {
0058:             let row_sub_array_info = SubArrayInfo::from_size(self.args.row_size);
0059:             let col_sub_array_info = SubArrayInfo::from_size(self.args.column_size);
0060: 
0061:             let row_col_sub_array = 
0062:                 self.add_module(BitcellArrayRecursiveArg::new(row_sub_array_info.size, col_sub_array_info.size), factory)?;
0063: 
0064:             for row in 0..row_sub_array_info.count {
0065:                 for col in 0..col_sub_array_info.count {
0066:                     let mut nets = vec![];
0067:                     let base_row_index = row * row_sub_array_info.size;
0068:                     let base_col_index = col * col_sub_array_info.size;
0069:                     
0070:                     for i in 0..col_sub_array_info.size {
0071:                         nets.push(Self::bitline_pn(base_col_index + i));
0072:                     }
0073: 
0074:                     for i in 0..col_sub_array_info.size {
0075:                         nets.push(Self::bitline_bar_pn(base_col_index + i));
0076:                     }
0077: 
0078:                     for i in 0..row_sub_array_info.size {
0079:                         nets.push(Self::wordline_pn(base_row_index + i));
0080:                     }
0081:                     
0082:                     nets.push(Self::vdd_pn());
0083:                     nets.push(Self::gnd_pn());
0084: 
0085:                     let inst_name = format_shr!("rowcol_subarray_{}_{}", row, col);
0086:                     let instance = self.add_instance(inst_name, row_col_sub_array.clone())?;
0087:                     self.connect_instance(instance, nets.into_iter())?;
0088:                 }
0089:             }
0090: 
0091:             if row_sub_array_info.remainder >= 1 {
0092:                 let row_sub_array = self.add_module(BitcellArrayRecursiveArg::new(row_sub_array_info.remainder, col_sub_array_info.size), factory)?;
0093:                 let base_row_index = row_sub_array_info.count * row_sub_array_info.size;
0094:                 for col in 0..col_sub_array_info.count {
0095:                     let mut nets = vec![];
0096:                     let base_col_index = col * col_sub_array_info.size;
0097: 
0098:                     for i in 0..col_sub_array_info.size {
0099:                         nets.push(Self::bitline_pn(base_col_index + i));
0100:                     }
0101: 
0102:                     for i in 0..col_sub_array_info.size {
0103:                         nets.push(Self::bitline_bar_pn(base_col_index + i));
0104:                     }
0105: 
0106:                     for i in 0..row_sub_array_info.remainder {
0107:                         nets.push(Self::wordline_pn(base_row_index + i));
0108:                     }
0109: 
0110:                     nets.push(Self::vdd_pn());
0111:                     nets.push(Self::gnd_pn());
0112: 
0113:                     let inst_name = format_shr!("row_subarray_{}", col);
0114:                     let instance = self.add_instance(inst_name, row_sub_array.clone())?;
0115:                     self.connect_instance(instance, nets.into_iter())?;
0116:                 }
0117:             }
0118: 
0119:             if col_sub_array_info.remainder >= 1 {
0120:                 let col_sub_array = self.add_module(BitcellArrayRecursiveArg::new(row_sub_array_info.size, col_sub_array_info.remainder), factory)?;
0121:                 let base_col_index = col_sub_array_info.count * col_sub_array_info.size;
0122:                 for row in 0..row_sub_array_info.count {
0123:                     let mut nets = vec![];
0124:                     let base_row_index = row * row_sub_array_info.size;
0125: 
0126:                     for i in 0..col_sub_array_info.remainder {
0127:                         nets.push(Self::bitline_pn(base_col_index + i));
0128:                     }
0129: 
0130:                     for i in 0..col_sub_array_info.remainder {
0131:                         nets.push(Self::bitline_bar_pn(base_col_index + i));
0132:                     }
0133: 
0134:                     for i in 0..row_sub_array_info.size {
0135:                         nets.push(Self::wordline_pn(base_row_index + i));
0136:                     }
0137: 
0138:                     nets.push(Self::vdd_pn());
0139:                     nets.push(Self::gnd_pn());
0140: 
0141:                     let inst_name = format_shr!("col_subarray_{}", row);
0142:                     let instance = self.add_instance(inst_name, col_sub_array.clone())?;
0143:                     self.connect_instance(instance, nets.into_iter())?;
0144:                 }
0145: 
0146:             }
0147: 
0148:             if row_sub_array_info.remainder >= 1 && col_sub_array_info.remainder >= 1 {
0149:                 let sub_array = self.add_module(BitcellArrayRecursiveArg::new(row_sub_array_info.remainder, col_sub_array_info.remainder), factory)?;
0150:                 let base_row_index = row_sub_array_info.count * row_sub_array_info.size;
0151:                 let base_col_index = col_sub_array_info.count * col_sub_array_info.size;
0152: 
0153:                 let mut nets = vec![];
0154: 
0155:                 for i in 0..col_sub_array_info.remainder {
0156:                     nets.push(Self::bitline_pn(base_col_index + i));
0157:                 }
0158: 
0159:                 for i in 0..col_sub_array_info.remainder {
0160:                     nets.push(Self::bitline_bar_pn(base_col_index + i));
0161:                 }
0162: 
0163:                 for i in 0..row_sub_array_info.remainder {
0164:                     nets.push(Self::wordline_pn(base_row_index + i));
0165:                 }
0166: 
0167:                 nets.push(Self::vdd_pn());
0168:                 nets.push(Self::gnd_pn());
0169: 
0170:                 let instance = self.add_instance("subarray", sub_array.clone())?;
0171:                 self.connect_instance(instance, nets.into_iter())?;
0172:             };
0173:         } else {
0174:             for row in 0..self.args.row_size {
0175:                 for col in 0..self.args.column_size {
0176:                     self.link_bitcell_instance(
0177:                         factory, 
0178:                         format!("bitcell_{}_{}", row, col), 
0179:                         Self::bitline_pn(col), 
0180:                         Self::bitline_bar_pn(col),
0181:                         Self::wordline_pn(row), 
0182:                         Self::vdd_pn(),
0183:                         Self::gnd_pn(),
0184:                     )?;
0185:                 }
0186:             }
0187:         }
0188: 
0189:         Ok(())
0190:     }
0191: 
0192: }
0193: 
0194: struct SubArrayInfo {   
0195:     pub size: usize,
0196:     pub count: usize,
0197:     pub remainder: usize,
0198: }
0199: 
0200: impl SubArrayInfo {
0201:     fn new(size: usize, count: usize, remainder: usize) -> Self {
0202:         Self { size, count, remainder }
0203:     }
0204: 
0205:     fn from_size(size: usize) -> Self {
0206:         let mut multiple = 1;
0207:         let mut left = size - multiple * multiple;
0208:         loop {
0209:             let next_multiple = multiple + 1;
0210:             let pow_mul2 = next_multiple * next_multiple;
0211:             if pow_mul2 > size { 
0212:                 break;
0213:             }
0214:             // Update
0215:             multiple = next_multiple;
0216:             left = size - pow_mul2;
0217:         }
0218: 
0219:         // Now size = multiple * multiple + remainder
0220:         // But, left may bigger than multiple
0221:         if multiple > left {
0222:             return SubArrayInfo::new(multiple, multiple, left);
0223:         }
0224:         
0225:         let remainder = left % multiple;
0226:         let subsize = multiple + (left / multiple);
0227:         let subcount = multiple;
0228: 
0229:         SubArrayInfo::new(subsize, subcount, remainder)
0230:     }
0231: }

// File: YouRAM-master\src\circuit\module\buffer.rs

0001: use youram_macro::module;
0002: use crate::{circuit::{CircuitFactory, DriveStrength, LogicGateKind}, YouRAMResult};
0003: 
0004: #[module(
0005:     input:  ("A", Input),
0006:     output: ("Z", Output),
0007:     vdd:    ("vdd", Vdd),
0008:     gnd:    ("gnd", Gnd),
0009: )]
0010: pub struct Buffer {
0011:     pub strength: DriveStrength,
0012: }
0013: 
0014: impl Buffer {
0015:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0016:         let inv = self.add_logicgate(LogicGateKind::Inv, self.args.strength, factory)?;
0017:         self.link_inv_instance("inv1", inv.clone(), [Self::input_pn(), "Z_bar".into(), Self::vdd_pn(), Self::gnd_pn()])?;
0018:         self.link_inv_instance("inv2", inv.clone(), ["Z_bar".into(), Self::output_pn(), Self::vdd_pn(), Self::gnd_pn()])?;
0019:         Ok(())
0020:     }
0021: }

// File: YouRAM-master\src\circuit\module\columnmux.rs

0001: use youram_macro::module;
0002: use crate::{circuit::CircuitFactory, format_shr, YouRAMResult};
0003: 
0004: #[module(
0005:     select:               ("sel{select_size}", Input),
0006:     bitline:              ("bl{select_size}", InOut),
0007:     bitline_bar:          ("br{select_size}", InOut),
0008:     bitline_selected:     ("bl", InOut),
0009:     bitline_bar_selected: ("br", InOut),
0010:     vdd:                  ("vdd", Vdd),
0011:     gnd:                  ("gnd", Gnd),
0012: )]
0013: pub struct ColumnMux {
0014:     pub select_size: usize,
0015: }
0016: 
0017: impl ColumnMux {
0018:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0019:         for i in 0..self.args.select_size {
0020:             self.link_column_trigate_instance(
0021:                 factory, 
0022:                 format_shr!("column_mux_{}", i), 
0023:                 Self::bitline_pn(i),
0024:                 Self::bitline_bar_pn(i), 
0025:                 Self::bitline_selected_pn(), 
0026:                 Self::bitline_bar_selected_pn(), 
0027:                 Self::select_pn(i), 
0028:                 Self::vdd_pn(),
0029:                 Self::gnd_pn()
0030:             )?;
0031:         }
0032:         Ok(())
0033:     }
0034: }

// File: YouRAM-master\src\circuit\module\columnmuxarray.rs

0001: use youram_macro::module;
0002: 
0003: use crate::{circuit::CircuitFactory, format_shr, YouRAMResult};
0004: 
0005: use super::ColumnMuxArg;
0006: 
0007: #[module(
0008:     select:               ("sel{select_size}", Input),
0009: 
0010:     bitline:              ("bl{mux_size}_{select_size}", InOut),
0011:     bitline_bar:          ("br{mux_size}_{select_size}", InOut),
0012: 
0013:     bitline_selected:     ("bl{mux_size}", InOut),
0014:     bitline_bar_selected: ("br{mux_size}", InOut),
0015:  
0016:     vdd:                  ("vdd", Vdd),
0017:     gnd:                  ("gnd", Gnd),
0018: )]
0019: pub struct ColumnMuxArray {
0020:     pub select_size: usize,
0021:     pub mux_size: usize,
0022: }
0023: 
0024: impl ColumnMuxArray {
0025:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0026:         let mux = self.add_module(ColumnMuxArg::new(self.args.select_size), factory)?;
0027:         for mux_index in 0..self.args.mux_size {
0028:             let mut nets = vec![];
0029:             
0030:             for sel_index in 0..self.args.select_size {
0031:                 nets.push(Self::select_pn(sel_index));
0032:             }
0033: 
0034:             for sel_index in 0..self.args.select_size {
0035:                 nets.push(Self::bitline_pn(mux_index, sel_index));
0036:             }
0037: 
0038:             for sel_index in 0..self.args.select_size {
0039:                 nets.push(Self::bitline_bar_pn(mux_index, sel_index));
0040:             }
0041:         
0042:             nets.push(Self::bitline_selected_pn(mux_index));
0043:             nets.push(Self::bitline_bar_selected_pn(mux_index));
0044: 
0045:             nets.push(Self::vdd_pn());
0046:             nets.push(Self::gnd_pn());
0047: 
0048:             self.link_module_instance(format_shr!("mux{}", mux_index), mux.clone(), nets.into_iter())?;
0049:         }
0050: 
0051:         Ok(())
0052:     }
0053: }

// File: YouRAM-master\src\circuit\module\controllogic.rs

0001: use youram_macro::module;
0002: use crate::{circuit::{CircuitFactory, DriveStrength, LogicGateKind}, YouRAMResult};
0003: 
0004: #[module(
0005:     clock:                 ("clk", Input),
0006:     chip_sel_bar:          ("csb", Input),
0007:     write_enable:          ("we", Input),
0008:     replical_bitline:      ("rbl", InOut),
0009: 
0010:     wordline_enable:       ("wl_en", Output),
0011:     precharge_enable_bar:  ("p_en_bar", Output),
0012:     sense_amp_enable:      ("sa_en", Output),
0013:     write_deriver_enable:  ("we_en", Output),
0014:     
0015:     voltage:               ("vdd", Vdd),
0016:     groud:                 ("gnd", Gnd),
0017: )]
0018: pub struct ControlLogic {
0019: }
0020: 
0021: impl ControlLogic {
0022:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0023:         // add circuits
0024:         let inv = self.add_logicgate(LogicGateKind::Inv, DRIVE_STRENGTH, factory)?;
0025:         let or2 = self.add_logicgate(LogicGateKind::Or(2), DRIVE_STRENGTH, factory)?;
0026:         let and2 = self.add_logicgate(LogicGateKind::And(2), DRIVE_STRENGTH, factory)?;
0027:         let and3 = self.add_logicgate(LogicGateKind::And(3), DRIVE_STRENGTH, factory)?;
0028:         let nand3 = self.add_logicgate(LogicGateKind::Nand(3), DRIVE_STRENGTH, factory)?;        
0029: 
0030:         // input inv
0031:         self.link_inv_instance("clk_inv", inv.clone(), [Self::clock_pn(), "clk_bar".into(), Self::voltage_pn(), Self::groud_pn()])?;
0032:         self.link_inv_instance("we_inv",  inv.clone(), [Self::write_enable_pn(), "we_bar".into(),  Self::voltage_pn(), Self::groud_pn()])?;
0033:         self.link_inv_instance("csb_inv", inv.clone(), [Self::chip_sel_bar_pn(), "csb_bar".into(), Self::voltage_pn(), Self::groud_pn()])?;
0034:         self.link_inv_instance("rbl_inv", inv.clone(), [Self::replical_bitline_pn(), "rbl_bar".into(), Self::voltage_pn(), Self::groud_pn()])?;
0035: 
0036:         // word line
0037:         self.link_logicgate_instance("wl_and2", and2.clone(), 
0038:             vec!["csb_bar".into(), Self::write_enable_pn()], "wl_net1", 
0039:             Self::voltage_pn(), Self::groud_pn())?;
0040:         self.link_logicgate_instance("wl_and3", and3.clone(), 
0041:             vec!["csb_bar".into(), "clk_bar".into(), Self::replical_bitline_pn()], "wl_net2", 
0042:             Self::voltage_pn(), Self::groud_pn())?;
0043:         self.link_logicgate_instance("wl_or2", or2.clone(), 
0044:             vec!["wl_net1", "wl_net2"], Self::wordline_enable_pn(), 
0045:             Self::voltage_pn(), Self::groud_pn())?;
0046: 
0047:         // precharge
0048:         self.link_logicgate_instance("p_nand3", nand3.clone(), 
0049:             vec!["csb_bar".into(), "we_bar".into(), Self::clock_pn()], Self::precharge_enable_bar_pn(), 
0050:             Self::voltage_pn(), Self::groud_pn())?;
0051: 
0052:         // sense amp
0053:         self.link_logicgate_instance("sa_and2_1", and2.clone(), 
0054:             vec!["csb_bar", "we_bar"], "sa_net1", 
0055:             Self::voltage_pn(), Self::groud_pn())?;
0056:         self.link_logicgate_instance("sa_and2_2", and2.clone(), 
0057:             vec!["clk_bar", "rbl_bar"], "sa_net2", 
0058:             Self::voltage_pn(), Self::groud_pn())?;
0059:         self.link_logicgate_instance("sa_and2", and2.clone(), 
0060:             vec!["sa_net1", "sa_net2"], Self::sense_amp_enable_pn(), 
0061:             Self::voltage_pn(), Self::groud_pn())?;
0062: 
0063:         // write deriver 
0064:         self.link_logicgate_instance("we_and2", and2.clone(), 
0065:             vec!["csb_bar".into(), Self::write_enable_pn()], Self::write_deriver_enable_pn(), 
0066:             Self::voltage_pn(), Self::groud_pn())?;
0067: 
0068:         Ok(())
0069:     }
0070: }
0071: 
0072: const DRIVE_STRENGTH: DriveStrength = DriveStrength::X1;
0073: // const POWER_DRIVE_STRENGTH: DriveStrength = DriveStrength::X2;

// File: YouRAM-master\src\circuit\module\core.rs

0001: use youram_macro::module;
0002: use crate::{check_arg, circuit::{AndArrayArg, Bank, BankArg, CircuitFactory, ControlLogic, ControlLogicArg}, format_shr, YouRAMResult};
0003: 
0004: #[module(
0005:     clock:         ("clk", Input),
0006:     chip_sel_bar:  ("csb", Input),
0007:     write_enable:  ("we", Input),
0008:     row_select:    ("rsel{row_size}", Input),
0009:     col_select:    ("csel{column_sel_size}", Input, "column_sel_size > 1"),
0010: 
0011:     data_input:    ("din{word_width}", Input),
0012:     data_output:   ("dout{word_width}", Input),
0013: 
0014:     vdd:           ("vdd", Vdd),
0015:     gnd:           ("gnd", Gnd),
0016: )]
0017: pub struct Core {
0018:     pub row_size: usize,
0019:     pub column_sel_size: usize,
0020:     pub word_width: usize,
0021: 
0022:     #[new(value = "column_sel_size * word_width")]
0023:     pub column_size: usize,
0024: }
0025: 
0026: impl Core {
0027:     pub const MAX_ROW_SIZE: usize = 64;
0028:     pub const MAX_COLUMN_SIZE: usize = 128;
0029:     pub const MAX_BITCELL_SIZE: usize = Self::MAX_ROW_SIZE * Self::MAX_COLUMN_SIZE;
0030: 
0031:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0032:         check_arg!(self.args.row_size <= Self::MAX_ROW_SIZE, "row sel size '{}' > {}", self.args.row_size, Self::MAX_ROW_SIZE);
0033:         check_arg!(self.args.column_sel_size <= Self::MAX_COLUMN_SIZE, "column sel size '{}' > {}", self.args.column_sel_size, Self::MAX_COLUMN_SIZE);
0034:         check_arg!(self.bitcell_size() <= Self::MAX_BITCELL_SIZE, "Too much bitcell size");
0035: 
0036:         let bank 
0037:             = self.add_module(BankArg::new(self.args.row_size, self.args.column_sel_size, self.args.word_width), factory)?;
0038:         let control_logic 
0039:             = self.add_module(ControlLogicArg::new(), factory)?;
0040:         let and_array 
0041:             = self.add_module(AndArrayArg::new(self.args.row_size), factory)?;
0042:         // TODO: wordline driver
0043: 
0044:         let rbl_net = ControlLogic::replical_bitline_pn();
0045:         let wl_en_net = ControlLogic::wordline_enable_pn();
0046:         let p_en_bar_net = ControlLogic::precharge_enable_bar_pn();
0047:         let sa_en_net = ControlLogic::sense_amp_enable_pn();
0048:         let we_en_net = ControlLogic::write_deriver_enable_pn();
0049: 
0050:         let wl_nets: Vec<_> = (0..self.args.row_size).map(|row| Bank::wordline_pn(row)).collect(); 
0051: 
0052:         // control_logic
0053:         {
0054:             let nets = vec![
0055:                 Self::clock_pn(),
0056:                 Self::chip_sel_bar_pn(),
0057:                 Self::write_enable_pn(),
0058:                 rbl_net.clone(),
0059:                 wl_en_net.clone(),
0060:                 p_en_bar_net.clone(),
0061:                 sa_en_net.clone(),
0062:                 we_en_net.clone(),
0063:                 Self::vdd_pn(),
0064:                 Self::gnd_pn(),
0065:             ];
0066:             self.link_module_instance("control_logic", control_logic, nets.into_iter())?;
0067:         }
0068: 
0069:         // bank
0070:         {
0071:             let mut nets = vec![
0072:                 wl_en_net.clone(),
0073:                 p_en_bar_net.clone(),
0074:                 sa_en_net.clone(),
0075:                 we_en_net.clone(),
0076:             ];
0077:             nets.extend(wl_nets.iter().cloned());
0078:             if bank.read().has_column_address() {
0079:                 nets.extend((0..self.args.column_sel_size).map(|c| Self::col_select_pn(c)));                
0080:             }
0081:             nets.extend((0..self.args.word_width).map(|i| Self::data_input_pn(i)));
0082:             nets.extend((0..self.args.word_width).map(|i| Self::data_output_pn(i)));
0083:             nets.push(rbl_net.clone());
0084:             nets.push(Self::vdd_pn());
0085:             nets.push(Self::gnd_pn());
0086: 
0087:             self.link_module_instance("bank", bank, nets.into_iter())?;
0088:         }
0089: 
0090:         // and array 
0091:         {
0092:             let mut nets = vec![];
0093:             nets.extend((0..self.args.row_size).map(|r| Self::row_select_pn(r)));
0094:             nets.push(wl_en_net.clone());
0095:             nets.extend(wl_nets.into_iter());
0096:             nets.push(Self::vdd_pn());
0097:             nets.push(Self::gnd_pn());
0098: 
0099:             self.link_module_instance("andarray", and_array, nets.into_iter())?;
0100:         }
0101: 
0102:         Ok(())
0103:     }
0104:     
0105:     pub fn bitcell_size(&self) -> usize {
0106:         self.args.row_size * self.args.column_sel_size
0107:     } 
0108: }

// File: YouRAM-master\src\circuit\module\coreselect.rs

0001: use youram_macro::module;
0002: use crate::{circuit::{CircuitFactory, DriveStrength, LogicGateKind}, format_shr, YouRAMResult};
0003: 
0004: use super::DecoderArg;
0005: 
0006: #[module(
0007:     chip_sel_bar:          ("csb", Input),
0008:     address:               ("addr{address_width}", Input),
0009:     data_output_core:      ("dout_core{core_size}[{word_width}]", Input),
0010:     chip_sel_bar_core:     ("csb_core{core_size}", Output),
0011:     data_output:           ("dout{word_width}", Output),
0012: 
0013:     vdd:                   ("vdd", Vdd),
0014:     gnd:                   ("gnd", Gnd),
0015: )]
0016: pub struct CoreSelector {
0017:     pub address_width: usize,
0018:     pub word_width: usize,
0019: 
0020:     #[new(value = "2usize.pow(address_width as u32)")]
0021:     pub core_size: usize,
0022: }
0023: 
0024: impl CoreSelector {
0025:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0026:         let decoder = self.add_module(DecoderArg::new(self.args.address_width), factory)?;
0027:         let inv = self.add_logicgate(LogicGateKind::Inv, DRIVE_STRENGHT, factory)?;
0028:         let or = self.add_logicgate(LogicGateKind::Or(2), DRIVE_STRENGHT, factory)?;
0029:         let and = self.add_logicgate(LogicGateKind::And(2), DRIVE_STRENGHT, factory)?;
0030:         let select_or = self.add_logicgate(LogicGateKind::Or(self.args.core_size), DRIVE_STRENGHT, factory)?;
0031: 
0032:         let y_nets: Vec<_> = (0..self.args.core_size).map(|i| format_shr!("y{}", i)).collect();
0033:         let ybar_nets: Vec<_> = (0..self.args.core_size).map(|i| format_shr!("ybar{}", i)).collect();
0034: 
0035:         // decoder: input `addr{}`, output `y{}`
0036:         {
0037:             let mut nets = vec![];
0038:             nets.extend((0..self.args.address_width).map(|i| Self::address_pn(i)));
0039:             nets.extend(y_nets.iter().cloned());
0040:             nets.push(Self::vdd_pn());
0041:             nets.push(Self::gnd_pn());
0042: 
0043:             self.link_module_instance("decoder", decoder, nets.into_iter())?;
0044:         }
0045: 
0046:         // add inv of all decoder select: input `y{}`, output `ybar{}`
0047:         for core_index in 0..self.args.core_size {
0048:             self.link_inv_instance(
0049:                 format_shr!("csb_inv{}", core_index), 
0050:                 inv.clone(), 
0051:                 [y_nets[core_index].clone(), ybar_nets[core_index].clone(), Self::vdd_pn(), Self::gnd_pn()]
0052:             )?;
0053:         }
0054: 
0055:         // add csb control: `csb` and with all `ybar{}` to control each chip `csb_core{}`
0056:         for core_index in 0..self.args.core_size {
0057:             self.link_logicgate_instance(
0058:                 format_shr!("csb_or{}", core_index), 
0059:                 or.clone(), 
0060:                 vec![Self::chip_sel_bar_pn(), ybar_nets[core_index].clone()],
0061:                 Self::chip_sel_bar_core_pn(core_index),
0062:                 Self::vdd_pn(),
0063:                 Self::gnd_pn(),
0064:             )?;
0065:         }
0066: 
0067:         // for each output bit, and select
0068:         for bit in 0..self.args.word_width {
0069:             // Select dout_core0[bit], dout_core0[bit], dout_core0[bit] .. as dout[bit]
0070:             // By y[0], y[1], y[2]... 
0071:             // if `y_nets{}` is 0(this chip not selected), data output should 0 
0072:             for core_index in 0..self.args.core_size {
0073:                 self.link_logicgate_instance(
0074:                     format_shr!("dout_and_{}_{}", core_index, bit), 
0075:                     and.clone(), 
0076:                     vec![y_nets[core_index].clone(), Self::data_output_core_pn(core_index, bit)],
0077:                     format_shr!("y_dout_core{}[{}]", core_index, bit),
0078:                     Self::vdd_pn(),
0079:                     Self::gnd_pn(), 
0080:                 )?;
0081:             }
0082: 
0083:             // add or gate, select a bit from `y_dout_core[0..][bit] to ``dout{bit}` 
0084:             self.link_logicgate_instance(
0085:                 format_shr!("dout_or{}", bit), 
0086:                 select_or.clone(), 
0087:                 (0..self.args.core_size).map(|core_index| format_shr!("y_dout_core{}[{}]", core_index, bit)).collect(),
0088:                 Self::data_output_pn(bit),
0089:                 Self::vdd_pn(),
0090:                 Self::gnd_pn(),
0091:             )?;
0092:         }
0093: 
0094:         Ok(())
0095:     }
0096: }
0097: 
0098: const DRIVE_STRENGHT: DriveStrength = DriveStrength::X1;

// File: YouRAM-master\src\circuit\module\datapath.rs

0001: use youram_macro::module;
0002: 
0003: use crate::{circuit::CircuitFactory, format_shr, YouRAMResult};
0004: 
0005: use super::{ColumnMuxArrayArg, SenseAmpArrayArg, WriteDriverArrayArg};
0006: 
0007: #[module(
0008:     sense_amp_enable:    ("sa_en", Input),
0009:     write_driver_enable: ("we_en", Input),
0010: 
0011:     bitline:             ("bl{column_size}", InOut),
0012:     bitline_bar:         ("br{column_size}", InOut),
0013: 
0014:     select:              ("sel{column_sel_size}", Input, "column_sel_size > 1"),
0015: 
0016:     data_input:          ("din{word_width}", Input),
0017:     data_output:         ("dout{word_width}", Input),
0018:     
0019:     vdd:                 ("vdd", Vdd),
0020:     gnd:                 ("gnd", Gnd),
0021: )]
0022: pub struct DataPath {
0023:     pub word_width: usize,
0024:     pub column_sel_size: usize,
0025: 
0026:     #[new(value = "word_width * column_sel_size")]
0027:     pub column_size: usize,
0028: }
0029: 
0030: impl DataPath {
0031:     /*
0032:     
0033:          |  | |  | |  | |  | |  | |  | |  | |  | |  | |  | |  | 
0034:         +----+----+----+----+----+----+----+----+----+----+----+
0035:         |    |    |    |   ...   |    |    |    |    |    |    |
0036:         | PR | PR | PR |   ...   | PR | PR | PR | PR | PR | PR |
0037:         |    |    |    |   ...   |    |    |    |    |    |    |
0038:         +----+----+----+----+----+----+----+----+----+----+----+
0039:          |  | |  | |  | |  | |  | |  | |  | |  | |  | |  | |  | 
0040:          |  | |  | |  | |  | |  | |  | |  | |  | |  | |  | |  | 
0041:         +----+----+----+----+----+----+----+----+----+----+----+
0042:         |    |    |    |   ...   |    |    |    |    |    |    |
0043:         | CL | CL | CL |   ...   | CL | CL | CL | CL | CL | CL |
0044:         |    |    |    |   ...   |    |    |    |    |    |    |
0045:         +----+----+----+----+----+----+----+----+----+----+----+ 
0046:          |  | |  | |  | |  | |  | |  | |  | |  | |  | |  | |  | 
0047:         --------------------------------------------------------
0048:         --------------------------------------------------------
0049:         --------------------------------------------------------
0050:         --------------------------------------------------------
0051:          |  |      |  |      |  |      |  |      |  |      |  |
0052:         +----+    +----+    +----+    +----+    +----+    +----+
0053:         |    |    |    |    |    |    |    |    |    |    |    |
0054:         |    |    |    |    |    |    |    |    |    |    |    |
0055:         | SA |    | SA |    | SA |    | SA |    | SA |    | SA |
0056:         |    |    |    |    |    |    |    |    |    |    |    |
0057:         |    |    |    |    |    |    |    |    |    |    |    |
0058:         +----+    +----+    +----+    +----+    +----+    +----+
0059:          |  |      |  |      |  |      |  |      |  |      |  |
0060:          |  +------------------------------------------------------
0061:          |  |      |  +--------------------------------------------
0062:          |  |      |  |      |  +----------------------------------
0063:          |  |      |  |      |  |      |  +------------------------
0064:          |  |      |  |      |  |      |  |      |  +--------------
0065:          |  |      |  |      |  |      |  |      |  |      |  +----
0066:          |  |      |  |      |  |      |  |      |  |      |  |
0067:         +----+    +----+    +----+    +----+    +----+    +----+
0068:         |    |    |    |    |    |    |    |    |    |    |    |
0069:         |    |    |    |    |    |    |    |    |    |    |    |
0070:         | WD |    | WD |    | WD |    | WD |    | WD |    | WD |
0071:         |    |    |    |    |    |    |    |    |    |    |    |
0072:         |    |    |    |    |    |    |    |    |    |    |    |
0073:         +----+    +----+    +----+    +----+    +----+    +----+
0074: 
0075:     */
0076:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0077:         let write_array = self.add_module(WriteDriverArrayArg::new(self.args.word_width, self.args.column_sel_size - 1), factory)?;
0078:         let senseamp_array = self.add_module(SenseAmpArrayArg::new(self.args.word_width, self.args.column_sel_size - 1), factory)?;
0079: 
0080:         let mut out_bl_nets = vec![];
0081:         let mut out_br_nets = vec![]; 
0082: 
0083:         if self.use_mux() {
0084:             let colmux_array = self.add_module(ColumnMuxArrayArg::new(self.args.column_sel_size, self.args.word_width), factory)?;
0085:             
0086:             for i in 0..self.args.word_width {
0087:                 out_bl_nets.push(format_shr!("out_bl{}", i));
0088:                 out_br_nets.push(format_shr!("out_br{}", i));
0089:             }
0090: 
0091:             let mut bl_nets = vec![];
0092:             let mut br_nets = vec![];
0093:             let mut coladdr_nets = vec![];
0094: 
0095:             for i in 0..self.args.column_sel_size {
0096:                 coladdr_nets.push(Self::select_pn(i));
0097:             }
0098: 
0099:             for i in 0..self.args.column_size {
0100:                 bl_nets.push(Self::bitline_pn(i));
0101:                 br_nets.push(Self::bitline_bar_pn(i));
0102:             }
0103: 
0104:             // create and mux array
0105:             let mut muxarray_nets = vec![];
0106: 
0107:             // "sel{select_size}"
0108:             for net in coladdr_nets.iter() {
0109:                 muxarray_nets.push(net.clone());
0110:             }
0111:             // "bl{mux_size}_{select_size}"
0112:             for mux in 0..self.args.word_width {
0113:                 for i in 0..self.args.column_sel_size {
0114:                     muxarray_nets.push(bl_nets[mux * self.args.column_sel_size + i].clone());
0115:                 }
0116:             }
0117:             // "br{mux_size}_{select_size}"
0118:             for mux in 0..self.args.word_width {
0119:                 for i in 0..self.args.column_sel_size {
0120:                     muxarray_nets.push(br_nets[mux * self.args.column_sel_size + i].clone());
0121:                 }
0122:             }
0123:             // "bl{mux_size}"
0124:             for mux in 0..self.args.word_width {
0125:                 muxarray_nets.push(out_bl_nets[mux].clone());
0126:             }
0127:             // "br{mux_size}"
0128:             for mux in 0..self.args.word_width {
0129:                 muxarray_nets.push(out_br_nets[mux].clone());
0130:             }
0131: 
0132:             muxarray_nets.push(Self::vdd_pn());
0133:             muxarray_nets.push(Self::gnd_pn());
0134:     
0135:             self.link_module_instance("colmux_array", colmux_array, muxarray_nets.into_iter())?;
0136:     
0137:         } else {
0138:             for word_index in 0..self.args.word_width {
0139:                 out_bl_nets.push(Self::bitline_pn(word_index));
0140:                 out_br_nets.push(Self::bitline_bar_pn(word_index));
0141:             }
0142:         }   
0143:         
0144:         // sense amp
0145:         {
0146:             let mut nets = vec![];
0147:             for word_index in 0..self.args.word_width {
0148:                 nets.push(out_bl_nets[word_index].clone());
0149:             }
0150:             for word_index in 0..self.args.word_width {
0151:                 nets.push(out_br_nets[word_index].clone());
0152:             }
0153:             for word_index in 0..self.args.word_width {
0154:                 nets.push(Self::data_output_pn(word_index));
0155:             }
0156:             nets.push(Self::sense_amp_enable_pn());
0157:             nets.push(Self::vdd_pn());
0158:             nets.push(Self::gnd_pn());
0159: 
0160:             self.link_module_instance("senseamp_array", senseamp_array, nets.into_iter())?;
0161:         }
0162: 
0163:         // write driver
0164:         {
0165:             let mut nets = vec![];
0166:             for word_index in 0..self.args.word_width {
0167:                 nets.push(Self::data_input_pn(word_index));
0168:             }
0169:             for word_index in 0..self.args.word_width {
0170:                 nets.push(out_bl_nets[word_index].clone());
0171:             }
0172:             for word_index in 0..self.args.word_width {
0173:                 nets.push(out_br_nets[word_index].clone());
0174:             }
0175:             nets.push(Self::write_driver_enable_pn());
0176:             nets.push(Self::vdd_pn());
0177:             nets.push(Self::gnd_pn());
0178: 
0179:             self.link_module_instance("write_array", write_array, nets.into_iter())?;
0180:         }
0181: 
0182:         Ok(())
0183:     }
0184: 
0185:     pub fn use_mux(&self) -> bool {
0186:         self.args.column_sel_size > 1
0187:     }
0188: }

// File: YouRAM-master\src\circuit\module\decoder.rs

0001: use youram_macro::module;
0002: use crate::{check_arg, circuit::{CircuitFactory, DriveStrength, LogicGateKind}, format_shr, ErrorContext, YouRAMResult};
0003: 
0004: const MAX_SIMPLE_INPUT_SIZE: usize = 4;
0005: const MIN_INPUT_SIZE: usize = 1;
0006: const MAX_INPUT_SIZE: usize = 12;
0007: const SUB_DECODERS_INPUT_SIZES: [&'static [usize]; 8] = [
0008:     &[2, 3], 
0009:     &[3, 3], 
0010:     &[3, 4], 
0011:     &[4, 4],
0012:     &[3, 3, 3],
0013:     &[3, 3, 4],
0014:     &[3, 4, 4],
0015:     &[4, 4, 4],
0016: ];
0017: 
0018: #[module(
0019:     address: ("A{input_size}", Input),
0020:     output:  ("Y{output_size}", Output),
0021:     vdd:     ("vdd", Vdd),
0022:     gnd:     ("gnd", Gnd),
0023: )]
0024: pub struct Decoder {
0025:     pub input_size: usize,
0026: 
0027:     #[new(value = "2usize.pow(input_size as u32)")]
0028:     pub output_size: usize,
0029: }
0030: 
0031: impl Decoder {
0032:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0033:         check_arg!(self.args.input_size >= MIN_INPUT_SIZE, "Input size '{}' < {}", self.args.input_size, MIN_INPUT_SIZE);
0034:         check_arg!(self.args.input_size <= MAX_INPUT_SIZE, "Input size '{}' > {}", self.args.input_size, MAX_INPUT_SIZE);
0035: 
0036:         match self.args.kind() {
0037:             DecoderType::OneAddr => self.build_one_addr(factory),
0038:             DecoderType::Simple => self.build_simple(factory),
0039:             DecoderType::Componet => self.build_componet(factory),
0040:         }
0041: 
0042:     }
0043: 
0044:     fn build_one_addr(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0045:         let inv = self.add_logicgate(LogicGateKind::Inv, DriveStrength::X2, factory)?;
0046:         
0047:         self.link_inv_instance("inv0", inv.clone(), [Self::address_pn(0), Self::output_pn(0), Self::vdd_pn(), Self::gnd_pn()])?;
0048:         self.link_inv_instance("inv1", inv.clone(), [Self::output_pn(0), Self::output_pn(1), Self::vdd_pn(), Self::gnd_pn()])?;
0049: 
0050:         Ok(())
0051:     }
0052: 
0053:     fn build_simple(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0054:         let inv = self.add_logicgate(LogicGateKind::Inv, DriveStrength::X2, factory)?;
0055:         let and = self.add_logicgate(LogicGateKind::And(self.args.input_size), DriveStrength::X2, factory)?;
0056:                         
0057:         let input_ports: Vec<_> = (0..self.args.input_size).map(|i| Self::address_pn(i)).collect();
0058:         let input_ports_bar: Vec<_> = (0..self.args.input_size).map(|i| format_shr!("A{}_bar", i)).collect();
0059: 
0060:         for i in 0..self.args.input_size {
0061:             let inst_name = format!("inv{}", i);
0062:             self.link_inv_instance(
0063:                 inst_name, inv.clone(), 
0064:                 [input_ports[i].clone(), input_ports_bar[i].clone(), Self::vdd_pn(), Self::gnd_pn()]
0065:             )?;
0066:         } 
0067: 
0068:         for i in 0..self.args.output_size {
0069:             let mut input_nets = vec![];
0070:             // 'i' is the AND gate's index. Each AND gate's inputs are [A0/A0_int, A1/A1_int ... An/An_int]
0071:             // There are '_inputSize' inputs, and 'j' is the input port' index.
0072:             // No.'j' bit in 'i' decides the port for Aj is inverted or not.
0073:             // For example, i == 000, the inputs are [A0_int, A1_int, A2_int].
0074:             //              i == 010, the inputs are [A0_int, A1,     A2_int].
0075:             // ...
0076:             for j in 0..self.args.input_size {
0077:                 let bit_one = ((i >> j) & 0x1) != 0;
0078:                 input_nets.push( if bit_one { input_ports[j].clone() } else { input_ports_bar[j].clone() } );   
0079:             }
0080: 
0081:             let inst_name = format!("and{}", i);
0082:             self.link_logicgate_instance(
0083:                 inst_name, and.clone(), 
0084:                 input_nets, Self::output_pn(i), Self::vdd_pn(), Self::gnd_pn()
0085:             )?;
0086:         }
0087: 
0088:         Ok(())
0089:     }
0090: 
0091:     fn build_componet(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0092:         let sub_decoders_input_size = self.sub_decoders_input_size();
0093:         let and = self.add_logicgate(LogicGateKind::And(sub_decoders_input_size.len()), DriveStrength::X2, factory).context("create and")?;
0094: 
0095:         let mut global_input_index = 0;
0096:         for (decoder_index, &sub_input_size) in sub_decoders_input_size.iter().enumerate() {
0097:             let arg = DecoderArg::new(sub_input_size);
0098:             let sub_decoder = self.add_module(arg, factory)?;
0099: 
0100:             // add nets
0101:             let mut nets = vec![];
0102:             for _ in 0..sub_input_size {
0103:                 nets.push(Self::address_pn(global_input_index));
0104:                 global_input_index += 1;
0105:             }
0106:             for ouput_index in 0..2usize.pow(sub_input_size as u32) { 
0107:                 nets.push(format_shr!("Y_{}_{}", decoder_index, ouput_index));
0108:             }
0109:             nets.push(Self::vdd_pn());
0110:             nets.push(Self::gnd_pn());
0111: 
0112:             let inst_name = format!("decoder{}", decoder_index);
0113:             let instance = self.add_instance(inst_name.clone(), sub_decoder)?;
0114:             self.connect_instance(instance, nets.into_iter())?;
0115:         }
0116: 
0117:         // for output AND
0118:         for and_index in 0..self.args.output_size {
0119:             let mut input_nets = vec![];
0120:             // For each decoder, get an output line as AND gate's input
0121:             // emmm... this algo is hard to explain, so TODO!!!
0122:             for (decoder_index, &_) in sub_decoders_input_size.iter().enumerate() {
0123:                 let mut prefix_sum = 0;
0124:                 for index in 0..decoder_index {
0125:                     prefix_sum += sub_decoders_input_size[index];
0126:                 }
0127:                 
0128:                 let mut mask = 1;
0129:                 for _ in 1..sub_decoders_input_size[decoder_index] {
0130:                     mask = (mask << 1) + 1;
0131:                 }
0132: 
0133:                 let decoder_output_index = (and_index >> prefix_sum) & mask;
0134:                 input_nets.push(format_shr!("Y_{}_{}", decoder_index, decoder_output_index));
0135:             }   
0136: 
0137:             self.link_logicgate_instance(format!("and{}", and_index), and.clone(), 
0138:                 input_nets, Self::output_pn(and_index), Self::vdd_pn(), Self::gnd_pn()
0139:             )?;
0140:         }
0141: 
0142:         Ok(())
0143:     }
0144: 
0145:     fn sub_decoders_index(&self) -> usize {
0146:         self.args.input_size - 1 - MAX_SIMPLE_INPUT_SIZE
0147:     } 
0148: 
0149:     fn sub_decoders_input_size(&self) -> &'static [usize] {
0150:         SUB_DECODERS_INPUT_SIZES[self.sub_decoders_index()]
0151:     }
0152: }
0153: 
0154: pub enum DecoderType {
0155:     OneAddr,
0156:     Simple,
0157:     Componet,
0158: }
0159: 
0160: impl DecoderArg {
0161:     pub fn kind(&self) -> DecoderType {
0162:         match self.input_size {
0163:             1 => DecoderType::OneAddr,
0164:             i if i <= MAX_SIMPLE_INPUT_SIZE => DecoderType::Simple,
0165:             _ => DecoderType::Componet,
0166:         }
0167:     }
0168: }

// File: YouRAM-master\src\circuit\module\fanoutbuffer.rs

0001: use youram_macro::module;
0002: use crate::{check_arg, circuit::{CircuitFactory, DriveStrength, ShrString, LogicGateKind}, format_shr, YouRAMResult};
0003: 
0004: const MAX_FANOUT_SIZE: usize = 1024;
0005: 
0006: #[module(
0007:     input:  ("in", Input),
0008:     output: ("out{fanout_size}", Output),
0009:     vdd:    ("vdd", Vdd),
0010:     gnd:    ("gnd", Gnd),
0011: )]
0012: pub struct FanoutBuffer {
0013:     pub fanout_size: usize,
0014: }
0015: 
0016: impl FanoutBuffer {
0017:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0018:         check_arg!(self.args.fanout_size > 1, "Fanout width '{}' <= 1", self.args.fanout_size);
0019: 
0020:         let mut output_size = self.args.fanout_size;
0021:         let mut tree_level_fanout_infos = vec![];
0022:         loop {
0023:             let info = Self::calculate_fanout_info(output_size);
0024:             let inv_size = info.inv_size;
0025:             output_size = inv_size;
0026:             tree_level_fanout_infos.push(info);
0027:             if inv_size == 1 {
0028:                 break;
0029:             }
0030:         }
0031: 
0032:         let inv = self.add_logicgate(LogicGateKind::Inv, DriveStrength::X2, factory)?;
0033:         let tree_depth = tree_level_fanout_infos.len();
0034: 
0035:         let tree_level_inv_name = |level: usize, inv_index: usize| -> ShrString {
0036:             format_shr!("inv_{}_{}", level, inv_index)
0037:         };
0038:         let tree_level_inv_output_name = |level: usize, inv_index: usize| -> ShrString {
0039:             format_shr!("net_{}_{}", level, inv_index)
0040:         };
0041:         let tree_level_inv_input_name = |level: usize, inv_index: usize| -> ShrString {
0042:             if level == 0 {
0043:                 return "net_begin".into();
0044:             }
0045:             let fanout = tree_level_fanout_infos[level - 1].fanout;
0046:             tree_level_inv_output_name(level - 1, inv_index / fanout)
0047:         };
0048: 
0049:         for level in 0..tree_depth {
0050:             let inv_size = tree_level_fanout_infos[level].inv_size;
0051: 
0052:             // Now, this level has 'inputSize' input, each of input will generate 'fanout' output
0053:             for inv_index in 0..inv_size {
0054:                 let name = tree_level_inv_name(level, inv_index);
0055:                 let input_net = tree_level_inv_input_name(level, inv_index);
0056:                 let output_net = tree_level_inv_output_name(level, inv_index);
0057:                 
0058:                 self.link_inv_instance(name, inv.clone(), [input_net, output_net, Self::vdd_pn(), Self::gnd_pn()])?;
0059:             }
0060:         }
0061: 
0062:         if tree_depth % 2 != 0 {
0063:             self.link_inv_instance("inv", inv.clone(), [
0064:                 Self::input_pn(),
0065:                 tree_level_inv_input_name(0, 0),
0066:                 Self::vdd_pn(),
0067:                 Self::gnd_pn(),
0068:             ])?;
0069:         } else {
0070:             self.connect_nets(Self::input_pn(), tree_level_inv_input_name(0, 0));
0071:         }
0072: 
0073:         // connect output
0074:         for output_index in 0..self.args.fanout_size {
0075:             self.connect_nets(
0076:                 Self::output_pn(output_index),
0077:                 tree_level_inv_output_name(tree_depth, output_index)
0078:             );
0079:         }
0080: 
0081:         Ok(())
0082:     }
0083: 
0084: 
0085:     fn calculate_fanout_info(output_size: usize) -> FanoutInfo {
0086:         if output_size <= MAX_FANOUT_SIZE {
0087:             return FanoutInfo::new(1, output_size, 0);
0088:         }
0089: 
0090:         let mut inv_size = 2;
0091:         loop {
0092:             let remainder = output_size % inv_size == 0;
0093:             let fanout = output_size / inv_size + (if remainder { 0 } else { 1 });
0094:             if fanout > MAX_FANOUT_SIZE {
0095:                 inv_size += 1;
0096:                 continue;
0097:             }
0098: 
0099:             let delta = fanout * inv_size - output_size;
0100:             return FanoutInfo::new(inv_size, fanout, delta);
0101:         }
0102:     }
0103: }
0104: 
0105: #[derive(derive_new::new)]
0106: struct FanoutInfo {
0107:     inv_size: usize, 
0108:     fanout: usize, 
0109:     #[allow(unused)]
0110:     delta: usize,
0111: }

// File: YouRAM-master\src\circuit\module\inputdffs.rs

0001: use youram_macro::module;
0002: use crate::{circuit::{CircuitFactory, DriveStrength}, format_shr, YouRAMResult};
0003: 
0004: #[module(
0005:     clock:            ("clk", Input),
0006: 
0007:     chip_sel_bar:     ("csb", Input),
0008:     write_enable:     ("we", Input),
0009:     address:          ("addr{address_width}", Input),
0010:     data_input:       ("din{word_width}", Input),
0011: 
0012:     chip_sel_bar_reg: ("csb_r", Output),
0013:     write_enable_reg: ("we_r", Output),
0014:     address_reg:      ("addr_r{address_width}", Output),
0015:     data_input_reg:   ("din_r{word_width}", Output),
0016: 
0017:     vdd:              ("vdd", Vdd),
0018:     gnd:              ("gnd", Gnd),
0019: )]
0020: pub struct InputDffs {
0021:     pub address_width: usize,
0022:     pub word_width: usize,
0023: }
0024: 
0025: impl InputDffs {
0026:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0027:         let dff = self.add_dff(DriveStrength::X1, factory)?;
0028: 
0029:         self.link_dff_instance("we_dff", dff.clone(), Self::write_enable_pn(), Self::clock_pn(), Self::write_enable_reg_pn(), "we_qn", Self::vdd_pn(), Self::gnd_pn())?;
0030:         self.link_dff_instance("csb_dff", dff.clone(), Self::chip_sel_bar_pn(), Self::clock_pn(), Self::chip_sel_bar_reg_pn(), "csb_qn", Self::vdd_pn(), Self::gnd_pn())?;
0031: 
0032:         for address in 0..self.args.address_width {
0033:             self.link_dff_instance(
0034:                 format_shr!("add_dff{}", address), dff.clone(), 
0035:                 Self::address_pn(address), 
0036:                 Self::clock_pn(), 
0037:                 Self::address_reg_pn(address), 
0038:                 format_shr!("addr{}_qn", address), 
0039:                 Self::vdd_pn(), 
0040:                 Self::gnd_pn()
0041:             )?;
0042:         }
0043: 
0044:         for address in 0..self.args.word_width {
0045:             self.link_dff_instance(
0046:                 format_shr!("din_dff{}", address), dff.clone(), 
0047:                 Self::data_input_pn(address), 
0048:                 Self::clock_pn(), 
0049:                 Self::data_input_reg_pn(address), 
0050:                 format_shr!("din{}_qn", address), 
0051:                 Self::vdd_pn(), 
0052:                 Self::gnd_pn()
0053:             )?;
0054:         }
0055: 
0056:         Ok(())
0057:     }
0058: }

// File: YouRAM-master\src\circuit\module\prechargearray.rs

0001: use youram_macro::module;
0002: use crate::{circuit::CircuitFactory, format_shr, YouRAMResult};
0003: 
0004: #[module(
0005:     bitline:       ("bl{column_size}", InOut),
0006:     bitline_bar:   ("br{column_size}", InOut),
0007:     enable:        ("p_en_bar", Input),
0008:     vdd:           ("vdd", Vdd),
0009: )]
0010: pub struct PrechargeArray {
0011:     pub column_size: usize,
0012: }
0013: 
0014: impl PrechargeArray {
0015:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0016:         for sa_index in 0..self.args.column_size {
0017:             self.link_precharge_instance(
0018:                 factory, 
0019:                 format_shr!("precharge{}", sa_index), 
0020:                 Self::bitline_pn(sa_index), 
0021:                 Self::bitline_bar_pn(sa_index), 
0022:                 Self::enable_pn(), 
0023:                 Self::vdd_pn(), 
0024:             )?;  
0025:         }
0026: 
0027:         Ok(())
0028:     }
0029: }

// File: YouRAM-master\src\circuit\module\replicalbitcellarray.rs

0001: use youram_macro::module;
0002: use crate::{check_arg, circuit::CircuitFactory, format_shr, YouRAMResult};
0003: 
0004: #[module(
0005:     replical_bitline:     ("rbl", InOut),
0006:     replical_bitline_bar: ("rbr", InOut),
0007:     wordline_enbale:      ("wl", Input),
0008:     vdd:                  ("vdd", Vdd),
0009:     gnd:                  ("gnd", Gnd),
0010: )]
0011: pub struct ReplicaBitcellArray {
0012:     pub bitcell_size: usize,
0013: }
0014: 
0015: const LINKED_BITCELL_SIZE: usize = 2;
0016: 
0017: impl ReplicaBitcellArray {
0018:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0019:         check_arg!(self.args.bitcell_size >= LINKED_BITCELL_SIZE, "Bitcell size < {}", LINKED_BITCELL_SIZE);
0020:         
0021:         for bitcell_index in 0..self.args.bitcell_size {
0022:             self.link_bitcell_instance(
0023:                 factory, 
0024:                 format_shr!("bitcell{}", bitcell_index), 
0025:                 Self::replical_bitline_pn(),
0026:                 Self::replical_bitline_bar_pn(),
0027:                 if bitcell_index < LINKED_BITCELL_SIZE { Self::wordline_enbale_pn() } else { Self::gnd_pn() }, 
0028:                 Self::vdd_pn(), 
0029:                 Self::gnd_pn(),
0030:             )?;
0031:         }
0032: 
0033:         Ok(())
0034:     }
0035: }

// File: YouRAM-master\src\circuit\module\senseamparray.rs

0001: use youram_macro::module;
0002: use crate::{circuit::CircuitFactory, format_shr, YouRAMResult};
0003: 
0004: #[module(
0005:     bitline:       ("bl{column_size}", InOut),
0006:     bitline_bar:   ("br{column_size}", InOut),
0007:     data_output:   ("dout{column_size}", Output),
0008:     enable:        ("sa_en", Input),
0009:     vdd:           ("vdd", Vdd),
0010:     gnd:           ("gnd", Gnd),
0011: )]
0012: pub struct SenseAmpArray {
0013:     pub column_size: usize,
0014:     pub spare_column_size: usize,
0015: }
0016: 
0017: impl SenseAmpArray {
0018:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0019:         for sa_index in 0..self.args.column_size {
0020:             self.link_senseamp_instance(
0021:                 factory, 
0022:                 format_shr!("sense_amp{}", sa_index), 
0023:                 Self::bitline_pn(sa_index), 
0024:                 Self::bitline_bar_pn(sa_index), 
0025:                 Self::data_output_pn(sa_index), 
0026:                 Self::enable_pn(), 
0027:                 Self::vdd_pn(), 
0028:                 Self::gnd_pn()
0029:             )?;  
0030:         }
0031: 
0032:         Ok(())
0033:     }
0034: }

// File: YouRAM-master\src\circuit\module\sram.rs

0001: use tracing::info;
0002: use youram_macro::module;
0003: use crate::{circuit::CircuitFactory, format_shr, YouRAMResult};
0004: use super::{Core, CoreArg, CoreSelector, CoreSelectorArg, DecoderArg, InputDffs, InputDffsArg};
0005: 
0006: #[module(
0007:     clock:         ("clk", Input),
0008:     chip_sel_bar:  ("csb", Input),
0009:     write_enable:  ("we", Input),
0010: 
0011:     address:       ("addr{address_width}", Input),
0012:     data_input:    ("din{word_width}", Input),
0013:     data_output:   ("dout{word_width}", Input),
0014: 
0015:     vdd:           ("vdd", Vdd),
0016:     gnd:           ("gnd", Gnd),
0017: )]
0018: pub struct Sram {
0019:     pub address_width: usize,
0020:     pub word_width: usize,
0021: 
0022:     #[new(value = "AddressDistribution::new(address_width, word_width)")]
0023:     pub distribution: AddressDistribution
0024: }
0025: 
0026: impl Sram {
0027:     const MAX_CORE_ADDRESS_WIDTH: usize = 2;
0028:     const MAX_CORE_SIZE: usize = 2usize.pow(Self::MAX_CORE_ADDRESS_WIDTH as u32);
0029:     const MAX_COLUMN_ADDRESS_WIDTH: usize = 3;
0030:     const MAX_BITCELL_SIZE: usize = Self::MAX_CORE_SIZE * Core::MAX_BITCELL_SIZE;
0031: 
0032:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0033:         info!("address distribution: {:?}", self.args.distribution);
0034:         // add module
0035:         let input_dffs 
0036:             = self.add_module(InputDffsArg::new(self.args.address_width, self.args.word_width), factory)?;
0037:         let core 
0038:             = self.add_module(CoreArg::new(self.core_row_size(), self.core_column_sel_size(), self.args.word_width), factory)?;
0039:         let row_decoder
0040:             = self.add_module(DecoderArg::new(self.row_address_width()), factory)?;
0041:         let column_decoder = if self.column_address_width() > 0 {
0042:             Some(self.add_module(DecoderArg::new(self.column_address_width()), factory)?)
0043:         } else {
0044:             None
0045:         };
0046: 
0047:         // golbal nets
0048:         let addr_reg_nets: Vec<_> = (0..self.args.address_width).map(|i| format_shr!("addr{}_r", i)).collect();
0049:         let addr_nets: Vec<_> = (0..self.args.address_width).map(|i| Self::address_pn(i)).collect();
0050: 
0051:         let din_reg_nets: Vec<_> = (0..self.args.word_width).map(|i| format_shr!("din{}_r", i)).collect();
0052:         let din_nets: Vec<_> = (0..self.args.word_width).map(|i| Self::data_input_pn(i)).collect();        
0053:         
0054:         let cbs_r_net = InputDffs::chip_sel_bar_reg_pn();
0055:         let we_r_net = InputDffs::write_enable_reg_pn();
0056:         
0057:         // input dff
0058:         {
0059:             let mut nets = vec![
0060:                 Self::clock_pn(),
0061:                 Self::chip_sel_bar_pn(),
0062:                 Self::write_enable_pn()
0063:             ];
0064:             nets.extend(addr_nets.iter().cloned());
0065:             nets.extend(din_nets.iter().cloned());
0066: 
0067:             nets.push(cbs_r_net.clone());
0068:             nets.push(we_r_net.clone());
0069:             nets.extend(addr_reg_nets.iter().cloned());
0070:             nets.extend(din_reg_nets.iter().cloned());
0071: 
0072:             nets.push(Self::vdd_pn());
0073:             nets.push(Self::gnd_pn());
0074:         
0075:             self.link_module_instance("input_dffs", input_dffs, nets.into_iter())?;
0076:         }
0077: 
0078:         // row address decoder
0079:         let rsel_nets: Vec<_> = (0..self.core_row_size()).map(|i| format_shr!("rsel{}", i)).collect();
0080:         {
0081:             let mut nets = vec![];
0082:             nets.extend((0..self.row_address_width()).map(|i| addr_reg_nets[i+self.column_address_width()].clone()));
0083:             nets.extend(rsel_nets.iter().cloned());
0084:             nets.push(Self::vdd_pn());
0085:             nets.push(Self::gnd_pn());
0086: 
0087:             self.link_module_instance("row_decoder", row_decoder, nets.into_iter())?;
0088:         }
0089: 
0090:         // col address decoder
0091:         let csel_nets = if let Some(col_decoder) = column_decoder.as_ref() {
0092:             let csel_nets: Vec<_> = (0..self.core_column_sel_size()).map(|i| format_shr!("csel{}", i)).collect();
0093:             let mut nets = vec![];
0094:             nets.extend((0..self.column_address_width()).map(|i| addr_reg_nets[i].clone()));
0095:             nets.extend(csel_nets.iter().cloned());
0096:             nets.push(Self::vdd_pn());
0097:             nets.push(Self::gnd_pn());
0098: 
0099:             self.link_module_instance("col_decoder", col_decoder.clone(), nets.into_iter())?;
0100:             csel_nets
0101:         } else {
0102:             vec![]
0103:         };
0104: 
0105:         // Sram core
0106:         if self.multiple_core() {
0107:             let core_sel
0108:                 = self.add_module(CoreSelectorArg::new(self.core_address_width(), self.args.word_width), factory)?;
0109: 
0110:             let core_csb_nets: Vec<_> = (0..self.core_count()).map(|c| CoreSelector::chip_sel_bar_core_pn(c)).collect();
0111:             let core_dout_nets = (0..self.core_count()).map(|core| {
0112:                 (0..self.args.word_width).map(move |bit| CoreSelector::data_output_core_pn(core, bit)).collect::<Vec<_>>()
0113:             }).collect::<Vec<_>>();
0114: 
0115:             // core select
0116:             {
0117:                 let mut nets = vec![];
0118:                 nets.push(cbs_r_net.clone());
0119:                 nets.extend((0..self.core_address_width()).map(|i| addr_reg_nets[i + self.column_address_width() + self.row_address_width()].clone()));
0120:                 nets.extend(core_dout_nets.iter().flatten().cloned());
0121:                 nets.extend(core_csb_nets.iter().cloned());
0122:                 nets.extend((0..self.args.word_width).map(|bit| Self::data_output_pn(bit)));
0123:                 nets.push(Self::vdd_pn());
0124:                 nets.push(Self::gnd_pn());
0125: 
0126:                 self.link_module_instance("core_selector", core_sel, nets.into_iter())?;
0127:             }
0128: 
0129:             // for each core
0130:             for core_index in 0..self.core_count() {
0131:                 let mut nets = vec![];
0132:                 nets.push(Self::clock_pn());
0133:                 nets.push(core_csb_nets[core_index].clone());
0134:                 nets.push(we_r_net.clone());
0135: 
0136:                 nets.extend(rsel_nets.iter().cloned());
0137:                 nets.extend(csel_nets.iter().cloned());
0138: 
0139:                 nets.extend(din_reg_nets.iter().cloned());
0140:                 nets.extend(core_dout_nets[core_index].iter().cloned());
0141: 
0142:                 nets.push(Self::vdd_pn());
0143:                 nets.push(Self::gnd_pn());
0144: 
0145:                 self.link_module_instance(format_shr!("core{}", core_index), core.clone(), nets.into_iter())?;
0146:             }
0147: 
0148:         } else {
0149:             let mut nets = vec![];
0150:             nets.push(Self::clock_pn());
0151:             nets.push(cbs_r_net.clone());
0152:             nets.push(we_r_net.clone());
0153: 
0154:             nets.extend(rsel_nets.iter().cloned());
0155:             nets.extend(csel_nets.iter().cloned());
0156: 
0157:             nets.extend(din_reg_nets.iter().cloned());
0158:             nets.extend((0..self.args.word_width).map(|bit| Self::data_output_pn(bit)));
0159: 
0160:             nets.push(Self::vdd_pn());
0161:             nets.push(Self::gnd_pn());
0162: 
0163:             self.link_module_instance("core", core.clone(), nets.into_iter())?;            
0164:         }
0165: 
0166:         Ok(())
0167:     }
0168: 
0169:     pub fn core_count(&self) -> usize {
0170:         2usize.pow(self.core_address_width() as u32)
0171:     }
0172: 
0173:     pub fn core_row_size(&self) -> usize {
0174:         2usize.pow(self.row_address_width() as u32)
0175:     }
0176: 
0177:     pub fn core_column_sel_size(&self) -> usize {
0178:         2usize.pow(self.column_address_width() as u32)
0179:     }
0180: 
0181:     pub fn core_column_size(&self) -> usize {
0182:         2usize.pow(self.column_address_width() as u32) * self.args.word_width
0183:     }
0184: 
0185:     pub fn multiple_core(&self) -> bool {
0186:         self.core_address_width() > 0
0187:     }
0188: 
0189:     pub fn core_address_width(&self) -> usize {
0190:         self.args.distribution.core_address_width
0191:     }
0192: 
0193:     pub fn column_address_width(&self) -> usize {
0194:         self.args.distribution.column_address_width
0195:     }
0196: 
0197:     pub fn row_address_width(&self) -> usize {
0198:         self.args.distribution.row_address_width
0199:     }
0200: 
0201:     pub fn address_width(&self) -> usize {
0202:         self.args.address_width
0203:     }
0204: 
0205:     pub fn word_width(&self) -> usize {
0206:         self.args.word_width
0207:     }
0208: 
0209:     pub fn word_size(&self) -> usize {
0210:         2usize.pow(self.row_address_width() as u32)
0211:     }
0212: }
0213: 
0214: #[derive(Debug)]
0215: pub struct AddressDistribution {
0216:     pub core_address_width: usize,
0217:     pub row_address_width: usize,
0218:     pub column_address_width: usize,
0219: }
0220: 
0221: impl AddressDistribution {
0222:     pub fn new(address_width: usize, word_width: usize) -> Self {
0223:         let total_bits = 2usize.pow(address_width as u32) * word_width;
0224:         assert!(total_bits <= Sram::MAX_BITCELL_SIZE, "Bit-cell size '{}' out of range '{}'", total_bits, Sram::MAX_BITCELL_SIZE);
0225: 
0226:         for core_address_width in 0..=Sram::MAX_CORE_ADDRESS_WIDTH {
0227:             if let Some(column_address_width) = Self::try_one_core(address_width - core_address_width, word_width) {
0228:                 return Self { 
0229:                     core_address_width,
0230:                     column_address_width,
0231:                     row_address_width: address_width - column_address_width - core_address_width
0232:                 };
0233:             }
0234:         }
0235: 
0236:         panic!("Can't find valid address distribution for the option of (address width: {}, word width: {})", address_width, word_width);
0237:     }
0238: 
0239:     fn try_one_core(address_width: usize, word_width: usize) -> Option<usize> {
0240:         let mut array_config = vec![];
0241:         let max_col_address = Sram::MAX_COLUMN_ADDRESS_WIDTH.min(address_width - 1);
0242: 
0243:         // Generate all possible configs
0244:         for col_addr_width in 0..=max_col_address {
0245:             let row = 2usize.pow((address_width - col_addr_width) as u32);
0246:             let col = word_width * 2usize.pow(col_addr_width as u32);
0247:             let delta = if row > col { row - col } else { col - row };
0248:             array_config.push((row, col, col_addr_width, delta));
0249:         }  
0250: 
0251:         // Sort by delta 
0252:         array_config.sort_by(|left, right| left.3.cmp(&right.3));
0253: 
0254:         // Find first config with satisfy constraints
0255:         for (row, col, col_addr_width, _) in array_config {
0256:             if row <= Core::MAX_ROW_SIZE && col <= Core::MAX_COLUMN_SIZE {
0257:                 return Some(col_addr_width);
0258:             }
0259:         }
0260: 
0261:         None
0262:     }
0263: }

// File: YouRAM-master\src\circuit\module\wordlinederiver.rs

0001: use youram_macro::module;
0002: use crate::{check_arg, circuit::{CircuitFactory, DriveStrength}, YouRAMResult};
0003: 
0004: use super::BufferArg;
0005: 
0006: #[module(
0007:     wordline_input:  ("wl_in", Input),
0008:     wordline:        ("wl", Output),
0009:     vdd:             ("vdd", Vdd),
0010:     gnd:             ("gnd", Gnd),
0011: )]
0012: pub struct WordlineDriver {
0013:     pub fanout: usize,
0014: }
0015: 
0016: impl WordlineDriver {
0017:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0018:         check_arg!(self.args.fanout > 0, "Fanout size '{}' less than 1", self.args.fanout);
0019:         
0020:         let strength = match self.args.fanout {
0021:             fanout if fanout > 1 => DriveStrength::X1,
0022:             fanout if fanout > 16 => DriveStrength::X2,
0023:             _ => DriveStrength::X4,
0024:         };
0025: 
0026:         let buffer = self.add_module(BufferArg::new(strength), factory)?;
0027:         self.link_module_instance("buffer", buffer, [
0028:             Self::wordline_input_pn(),
0029:             Self::wordline_pn(),
0030:             Self::vdd_pn(),
0031:             Self::gnd_pn(),
0032:         ].into_iter())?;
0033: 
0034:         Ok(())
0035:     }
0036: }

// File: YouRAM-master\src\circuit\module\wordlinederiverarr.rs

0001: use youram_macro::module;
0002: use crate::{check_arg, circuit::CircuitFactory, format_shr, YouRAMResult};
0003: 
0004: use super::WordlineDriverArg;
0005: 
0006: #[module(
0007:     wordline_input: ("wl_in{wordline_size}", Input),
0008:     wordline:       ("wl{wordline_size}", Input),
0009:     vdd:            ("vdd", Vdd),
0010:     gnd:            ("gnd", Gnd),
0011: )]
0012: pub struct WordlineDriverArray {
0013:     pub fanout: usize,
0014:     pub wordline_size: usize,
0015: }
0016: 
0017: impl WordlineDriverArray {
0018:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0019:         check_arg!(self.args.fanout > 0, "Fanout size '{}' less than 1", self.args.fanout);
0020:         check_arg!(self.args.wordline_size > 0, "wordline size size '{}' less than 1", self.args.wordline_size);
0021: 
0022:         let wordline = self.add_module(WordlineDriverArg::new(self.args.fanout), factory)?;
0023:         for wordline_index in 0..self.args.wordline_size {
0024:             self.link_module_instance(
0025:                 format_shr!("wordline_driver{}", wordline_index), 
0026:                 wordline.clone(), [
0027:                     Self::wordline_input_pn(wordline_index),
0028:                     Self::wordline_input_pn(wordline_index),
0029:                     Self::vdd_pn(),
0030:                     Self::gnd_pn(),
0031:                 ].into_iter()
0032:             )?;
0033:         }
0034: 
0035:         Ok(())
0036:     }
0037: }

// File: YouRAM-master\src\circuit\module\writedriverarray.rs

0001: use youram_macro::module;
0002: use crate::{circuit::CircuitFactory, format_shr, YouRAMResult};
0003: 
0004: #[module(
0005:     data_input:          ("din{column_size}", Input),
0006:     bitline:             ("bl{column_size}", InOut),
0007:     bitline_bar:         ("br{column_size}", InOut),
0008:     enable:              ("we_en", Input),
0009:     vdd:                 ("vdd", Vdd),
0010:     gnd:                 ("gnd", Gnd),
0011: )]
0012: pub struct WriteDriverArray {
0013:     pub column_size: usize,
0014:     pub spare_column_size: usize,
0015: }
0016: 
0017: impl WriteDriverArray {
0018:     pub fn build(&mut self, factory: &mut CircuitFactory) -> YouRAMResult<()> {
0019:         for wd_index in 0..self.args.column_size {
0020:             self.link_writedriver_instance(
0021:                 factory, 
0022:                 format_shr!("write_driver{}", wd_index), 
0023:                 Self::data_input_pn(wd_index), 
0024:                 Self::bitline_pn(wd_index), 
0025:                 Self::bitline_bar_pn(wd_index), 
0026:                 Self::enable_pn(), 
0027:                 Self::vdd_pn(), 
0028:                 Self::gnd_pn()
0029:             )?;   
0030:         }
0031: 
0032:         Ok(())
0033:     }
0034: }

// File: YouRAM-master\src\circuit\primitive\leafcell.rs

0001: use reda_sp::Subckt;
0002: use crate::circuit::{Design, Port, Shr};
0003: 
0004: use super::Primitive;
0005: 
0006: pub enum Leafcell {
0007:     Bitcell(Bitcell),
0008:     SenseAmp(SenseAmp),
0009:     WriteDriver(WriteDriver),
0010:     ColumnTriGate(ColumnTriGate),
0011:     Precharge(Precharge),
0012: }
0013: 
0014: macro_rules! define_leafcell {
0015:     ($name:ident, $($port:ident),+ $(,)?) => {
0016:         pub struct $name {
0017:             $(pub $port: Shr<Port>,)+
0018:             pub ports: ::std::vec::Vec<Shr<Port>>,
0019:             pub netlist: Subckt,
0020:         }
0021: 
0022:         impl $name {
0023:             pub fn new($($port: Shr<Port>,)+ netlist: Subckt) -> Self {
0024:                 let ports = ::std::vec![ $($port.clone(),)+ ];
0025:                 Self { $($port,)+ ports, netlist }
0026:             }
0027:         }
0028: 
0029:         impl From<$name> for Leafcell {
0030:             fn from(value: $name) -> Self {
0031:                 Self::$name(value)
0032:             }
0033:         }
0034:     };
0035: }
0036: 
0037: pub const BITCELL_NAME: &'static str = "bitcell";
0038: pub const SENSE_AMP_NAME: &'static str = "sense_amp";
0039: pub const WRITE_DRIVER_NAME: &'static str = "write_driver";
0040: pub const COLUMN_TRI_GATE_NAME: &'static str = "column_trigate";
0041: pub const PRECHARGE_NAME: &'static str = "precharge";
0042: 
0043: define_leafcell!(Bitcell, bitline, bitline_bar, word_line, vdd, gnd);
0044: define_leafcell!(SenseAmp, bitline, bitline_bar, data_output, enable, vdd, gnd);
0045: define_leafcell!(WriteDriver, data_input, bitline, bitline_bar, enable, vdd, gnd);
0046: define_leafcell!(ColumnTriGate, bitline, bitline_bar, bitline_output, bitline_bar_output, select, vdd, gnd);
0047: define_leafcell!(Precharge, bitline, bitline_bar, enable, vdd);
0048: 
0049: impl Design for Leafcell {
0050:     fn name(&self) -> crate::circuit::ShrString {
0051:         match self {
0052:             Self::Bitcell(_) => BITCELL_NAME.into(),
0053:             Self::SenseAmp(_) => SENSE_AMP_NAME.into(),
0054:             Self::WriteDriver(_) => WRITE_DRIVER_NAME.into(),
0055:             Self::ColumnTriGate(_) => COLUMN_TRI_GATE_NAME.into(),
0056:             Self::Precharge(_) => PRECHARGE_NAME.into(),
0057:         }
0058:     }
0059: 
0060:     fn ports(&self) -> &[Shr<Port>] {
0061:         match self {
0062:             Self::Bitcell(l) => &l.ports,
0063:             Self::SenseAmp(l) => &l.ports,
0064:             Self::WriteDriver(l) => &l.ports,
0065:             Self::ColumnTriGate(l) => &l.ports,
0066:             Self::Precharge(l) => &l.ports,
0067:         }
0068:     }
0069: }
0070: 
0071: impl Primitive for Leafcell {
0072:     fn netlist(&self) -> &Subckt {
0073:         match self {
0074:             Self::Bitcell(l) => &l.netlist,
0075:             Self::SenseAmp(l) => &l.netlist,
0076:             Self::WriteDriver(l) => &l.netlist,
0077:             Self::ColumnTriGate(l) => &l.netlist,
0078:             Self::Precharge(l) => &l.netlist,
0079:         }
0080:     }
0081: }

// File: YouRAM-master\src\circuit\primitive\mod.rs

0001: mod leafcell;
0002: mod stdcell;
0003: 
0004: use std::sync::{Arc, RwLock};
0005: 
0006: pub use leafcell::*;
0007: pub use stdcell::*;
0008: use reda_sp::Subckt;
0009: use super::{Design, Shr};
0010: 
0011: pub trait Primitive : Design + Send + Sync {
0012:     fn netlist(&self) -> &Subckt;
0013: }
0014: 
0015: impl Into<Shr<dyn Primitive>> for Shr<LogicGate> {
0016:     fn into(self) -> Shr<dyn Primitive> {
0017:         let inner = self.inner();
0018:         let inner: Arc<RwLock<dyn Primitive>> = inner;
0019:         Shr::from_inner(inner)
0020:     }
0021: }
0022: 
0023: impl Into<Shr<dyn Primitive>> for Shr<Dff> {
0024:     fn into(self) -> Shr<dyn Primitive> {
0025:         let inner = self.inner();
0026:         let inner: Arc<RwLock<dyn Primitive>> = inner;
0027:         Shr::from_inner(inner)
0028:     }
0029: }
0030: 
0031: impl Into<Shr<dyn Primitive>> for Shr<Leafcell> {
0032:     fn into(self) -> Shr<dyn Primitive> {
0033:         let inner = self.inner();
0034:         let inner: Arc<RwLock<dyn Primitive>> = inner;
0035:         Shr::from_inner(inner)
0036:     }
0037: }

// File: YouRAM-master\src\circuit\primitive\stdcell.rs

0001: use std::fmt::Display;
0002: use reda_lib::model::{LibCell, LibTiming};
0003: use reda_sp::Subckt;
0004: use crate::circuit::{CircuitError, Design, Port, Shr, ShrString};
0005: use super::Primitive;
0006: 
0007: pub enum Stdcell {
0008:     LogicGate(LogicGate),
0009:     Dff(Dff),
0010: }
0011: 
0012: #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
0013: pub enum LogicGateKind {
0014:     Inv,
0015:     And(usize),
0016:     Or(usize),
0017:     Nand(usize),
0018:     Nor(usize),
0019: }
0020: 
0021: #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
0022: pub enum DriveStrength {
0023:     X1, X2, X4, X8, X16, X32,
0024: }
0025: 
0026: #[derive(Debug, PartialEq, Eq, Hash)]
0027: pub struct LogicGateArg {
0028:     pub kind: LogicGateKind,
0029:     pub strength: DriveStrength,
0030: }
0031: 
0032: pub struct LogicGate {
0033:     pub name: ShrString,
0034:     
0035:     pub drive_strength: DriveStrength,
0036:     pub kind: LogicGateKind,
0037: 
0038:     pub ports: Vec<Shr<Port>>,
0039:     pub input_port_indexs: Vec<usize>,
0040:     pub output_port_index: usize,
0041:     pub vdd_port_index: usize,
0042:     pub gnd_port_index: usize,
0043: 
0044:     pub netlist: Subckt,
0045: }
0046: 
0047: #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
0048: pub enum LogicGatePort {
0049:     Input(usize),
0050:     Output,
0051:     Vdd,
0052:     Gnd,
0053: }
0054: 
0055: pub struct Dff {
0056:     pub name: ShrString,
0057:     
0058:     pub drive_strength: DriveStrength,
0059: 
0060:     pub ports: Vec<Shr<Port>>,
0061:     pub din_port_index: usize,
0062:     pub clk_port_index: usize,
0063:     pub q_port_index: usize,
0064:     pub qn_port_index: usize,
0065:     pub vdd_port_index: usize,
0066:     pub gnd_port_index: usize,
0067: 
0068:     pub netlist: Subckt,
0069: 
0070:     pub hold_rising_timing: LibTiming,
0071:     pub setup_rising_timing: LibTiming,
0072: }
0073: 
0074: impl LogicGateArg {
0075:     pub fn new(kind: LogicGateKind, strength: DriveStrength) -> Self {
0076:         Self { kind, strength }
0077:     }
0078: }
0079: 
0080: impl LogicGate {
0081:     pub fn input_ports(&self) -> impl Iterator<Item = &Shr<Port>> {
0082:         self.ports.iter()
0083:             .filter(|port| port.read().is_input())
0084:     }
0085: 
0086:     pub fn output_ports(&self) -> impl Iterator<Item = &Shr<Port>> {
0087:         self.ports.iter()
0088:             .filter(|port| port.read().is_output())
0089:     }
0090: 
0091:     pub fn source_ports(&self) -> impl Iterator<Item = &Shr<Port>> {
0092:         self.ports.iter()
0093:             .filter(|port| port.read().is_source())
0094:     }
0095: 
0096:     pub fn input_pn(&self, order: usize) -> Result<ShrString, CircuitError> {
0097:         self.input_ports()
0098:             .nth(order)
0099:             .ok_or_else(|| CircuitError::LogicGateInputPortOutOfRange(order))
0100:             .map(|port| port.read().name.clone())
0101:     }
0102: 
0103:     pub fn output_pn(&self) -> ShrString {
0104:         self.output_ports()
0105:             .nth(0)
0106:             .map(|port| port.read().name.clone())
0107:             .unwrap()
0108:     }
0109: 
0110:     pub fn vdd_pn(&self) -> ShrString {
0111:         self.ports.iter()
0112:             .find(|port| port.read().is_vdd())
0113:             .map(|port| port.read().name.clone())
0114:             .unwrap()
0115:     }
0116: 
0117:     pub fn gnd_pn(&self) -> ShrString {
0118:         self.ports.iter()
0119:             .find(|port| port.read().is_gnd())
0120:             .map(|port| port.read().name.clone())
0121:             .unwrap()
0122:     }
0123: }
0124: 
0125: impl DriveStrength {
0126:     pub fn try_from_cell(cell: &LibCell) -> Option<Self> {
0127:         let name = &cell.name.to_lowercase();
0128:         if name.contains("x32") {
0129:             return Some(Self::X32);
0130:         }
0131:         if name.contains("x16") {
0132:             return Some(Self::X16);
0133:         }
0134:         if name.contains("x8") {
0135:             return Some(Self::X8);
0136:         }
0137:         if name.contains("x4") {
0138:             return Some(Self::X4);
0139:         }
0140:         if name.contains("x2") {
0141:             return Some(Self::X2);
0142:         }
0143:         if name.contains("x1") {
0144:             return Some(Self::X1);
0145:         }
0146:         None
0147:     }
0148: }
0149: 
0150: impl Display for DriveStrength {
0151:     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
0152:         match self {
0153:             Self::X1  => write!(f, "x1"),
0154:             Self::X2  => write!(f, "x2"),
0155:             Self::X4  => write!(f, "x4"),
0156:             Self::X8  => write!(f, "x8"),
0157:             Self::X16 => write!(f, "x16"),
0158:             Self::X32 => write!(f, "x32"),
0159:         }
0160:     }
0161: }
0162: 
0163: impl Display for LogicGateKind {
0164:     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
0165:         match self {
0166:             Self::Inv => write!(f, "inv"),
0167:             Self::And(size) => write!(f, "and{}", size),
0168:             Self::Nand(size) => write!(f, "nand{}", size),
0169:             Self::Or(size) => write!(f, "or{}", size),
0170:             Self::Nor(size) => write!(f, "nor{}", size),
0171:         }
0172:     }
0173: }
0174: 
0175: impl Design for LogicGate {
0176:     fn name(&self) -> ShrString {
0177:         self.name.clone()
0178:     }
0179: 
0180:     fn ports(&self) -> &[Shr<Port>] {
0181:         &self.ports
0182:     }
0183: }
0184: 
0185: impl Primitive for LogicGate {
0186:     fn netlist(&self) -> &Subckt {
0187:         &self.netlist
0188:     }
0189: }
0190: 
0191: impl Design for Dff {
0192:     fn name(&self) -> ShrString {
0193:         self.name.clone()
0194:     }
0195: 
0196:     fn ports(&self) -> &[Shr<Port>] {
0197:         &self.ports
0198:     }
0199: }
0200: 
0201: impl Primitive for Dff {
0202:     fn netlist(&self) -> &Subckt {
0203:         &self.netlist
0204:     }
0205: }
0206: 
0207: impl Design for Stdcell {
0208:     fn name(&self) -> ShrString {
0209:         match self {
0210:             Self::LogicGate(s) => s.name(),
0211:             Self::Dff(s) => s.name(),
0212:         }
0213:     }
0214: 
0215:     fn ports(&self) -> &[Shr<Port>] {
0216:         match self {
0217:             Self::LogicGate(s) => s.ports(),
0218:             Self::Dff(s) => s.ports(),
0219:         }
0220:     }
0221: }
0222: 
0223: impl Primitive for Stdcell {
0224:     fn netlist(&self) -> &Subckt {
0225:         match self {
0226:             Self::LogicGate(s) => s.netlist(),
0227:             Self::Dff(s) => s.netlist(),
0228:         }
0229:     }
0230: }

// File: YouRAM-master\src\export\liberty.rs

0001: use std::{fs::File, path::Path, sync::Arc};
0002: use reda_lib::model::LibLuTable;
0003: use reda_unit::{Capacitance, Temperature, Time, Voltage};
0004: use tracing::info;
0005: use std::io::{BufWriter, Write};
0006: use std::fmt::Write as FmtWrite;
0007: use crate::circuit::DriveStrength;
0008: use crate::{charz::TimingCharz, circuit::{Shr, Sram}, pdk::{Pdk, Process, Pvt}, simulate::SpiceCommand, YouRAMResult};
0009: 
0010: pub fn write_liberty(
0011:     sram: Shr<Sram>, 
0012:     path: impl AsRef<Path>, 
0013:     period: Time, 
0014:     pdk: Arc<Pdk>, 
0015:     command: Box<dyn SpiceCommand>, 
0016:     temp_folder: impl AsRef<Path>,
0017: ) -> YouRAMResult<()> {
0018:     // collect all 
0019:     let input_net_transitions = pdk.timing_input_net_transitions();
0020:     let output_net_capacitances = pdk.timing_output_net_capacitances();
0021:     let pvt = pdk.pvt();
0022:     let temp_folder: &Path = temp_folder.as_ref();
0023: 
0024:     let all_result = TimingCharz::config()
0025:         .sram(sram.clone())
0026:         .period(period)
0027:         .pvt(pvt.clone())
0028:         .input_net_transitions(input_net_transitions)
0029:         .output_net_capacitances(output_net_capacitances)
0030:         .pdk(pdk.clone())
0031:         .command_box(command)
0032:         .temp_folder(temp_folder)
0033:         .analyze()?;
0034: 
0035:     let mut delay_lhs = vec![];
0036:     let mut delay_hls = vec![];
0037:     let mut slew_lhs = vec![];
0038:     let mut slew_hls = vec![];
0039: 
0040:     for result_same_slew in all_result.into_iter() {
0041:         let mut delay_lh_in_same_input_slew = vec![];
0042:         let mut delay_hl_in_same_input_slew = vec![];
0043:         let mut slew_lh_in_same_input_slew = vec![];
0044:         let mut slew_hl_in_same_input_slew = vec![];
0045:         for result in result_same_slew.into_iter() {
0046:             delay_hl_in_same_input_slew.push(result.delay_hl);
0047:             delay_lh_in_same_input_slew.push(result.delay_lh);
0048:             slew_hl_in_same_input_slew.push(result.slew_hl);
0049:             slew_lh_in_same_input_slew.push(result.slew_lh);
0050:         }
0051:         delay_lhs.push(delay_hl_in_same_input_slew);
0052:         delay_hls.push(delay_lh_in_same_input_slew);
0053:         slew_lhs.push(slew_hl_in_same_input_slew);
0054:         slew_hls.push(slew_lh_in_same_input_slew);
0055:     }
0056: 
0057:     // write to path
0058:     let path = path.as_ref();
0059:     info!("write circuit {} to {:?}", sram.read().name, path);
0060:     let mut writor = LibertyWritor::new(
0061:         sram, 
0062:         pvt.clone(), 
0063:         pdk.clone(), 
0064:         input_net_transitions.to_vec(), 
0065:         output_net_capacitances.to_vec(), 
0066:         delay_lhs,
0067:         delay_hls,
0068:         slew_lhs,
0069:         slew_hls,
0070:         path,
0071:     )?;
0072:     writor.write()?;
0073: 
0074:     Ok(())
0075: }
0076: 
0077: struct LibertyWritor {
0078:     sram: Shr<Sram>,
0079:     pvt: Pvt,
0080:     pdk: Arc<Pdk>, 
0081:     input_net_transitions: Vec<Time>,
0082:     output_net_capacitances: Vec<Capacitance>,
0083:     delay_lhs: Vec<Vec<Time>>,
0084:     delay_hls: Vec<Vec<Time>>,
0085:     slew_lhs: Vec<Vec<Time>>,
0086:     slew_hls: Vec<Vec<Time>>,
0087:     writor: BufWriter<File>,
0088: }
0089: 
0090: impl LibertyWritor {
0091:     fn new(
0092:         sram: Shr<Sram>, 
0093:         pvt: Pvt, 
0094:         pdk: Arc<Pdk>, 
0095:         input_net_transitions: Vec<Time>,
0096:         output_net_capacitances: Vec<Capacitance>,
0097:         delay_lhs: Vec<Vec<Time>>,
0098:         delay_hls: Vec<Vec<Time>>,
0099:         slew_lhs: Vec<Vec<Time>>,
0100:         slew_hls: Vec<Vec<Time>>,
0101:         path: impl AsRef<Path>, 
0102:     ) -> YouRAMResult<Self> {
0103:         let file = File::create(path)?;
0104:         let writor = BufWriter::new(file);
0105: 
0106: 
0107: 
0108:         Ok(Self {
0109:             sram, pvt, pdk, input_net_transitions, output_net_capacitances, delay_hls, delay_lhs, slew_hls, slew_lhs, writor
0110:         })
0111:     }
0112: 
0113:     fn write(&mut self) -> YouRAMResult<()> {
0114:         self.write_library()?;
0115:         Ok(())
0116:     }
0117: 
0118:     fn write_library(&mut self) -> YouRAMResult<()> {
0119:         self.write_line(0, &format!("library ({}_lib) {{", self.pdk.name()))?;
0120:         self.write_line(1, "delay_model : \"table_lookup\";")?;
0121: 
0122:         self.write_units()?;
0123:         self.write_defaults()?;
0124:         self.write_luttemplate()?;
0125:         self.write_bus()?;
0126:         self.write_cell()?;
0127: 
0128:         self.write_line(0, "}")?;
0129:         Ok(())    
0130:     }
0131: 
0132:     fn write_cell(&mut self) -> YouRAMResult<()> {
0133:         self.write_line(1, &format!("cell ({}) {{", self.sram.read().name))?;
0134:         self.write_line(2, "memory() {")?;
0135:         self.write_line(3, "type : ram;")?;
0136:         self.write_line(3, &format!("address_width : {};", self.sram.read().address_width()))?;
0137:         self.write_line(3, &format!("word_width : {};", self.sram.read().word_width()))?;
0138:         self.write_line(2, "}")?; // memory
0139:         self.write_enter()?;
0140: 
0141:         self.write_line(2, "interface_timing : true;")?;
0142:         self.write_line(2, "dont_use  : true;")?;
0143:         self.write_line(2, "map_only   : true;")?;
0144:         self.write_line(2, "dont_touch : true;")?;
0145:         self.write_enter()?;
0146: 
0147:         self.write_pgpin()?;
0148:         self.write_word_bus()?;
0149:         self.write_address_bus()?;
0150:         self.write_control_pins()?;
0151: 
0152:         self.write_line(1, "}")?; // cell
0153: 
0154:         Ok(())
0155:     }
0156: 
0157:     fn write_units(&mut self) -> YouRAMResult<()> {
0158:         self.write_line(1, "time_unit : \"1ns\";")?;
0159:         self.write_line(1, "voltage_unit : \"1V\"")?;
0160:         self.write_line(1, "current_unit : \"1mA\";")?;
0161:         self.write_line(1, "resistance_unit : \"1kohm\";")?;
0162:         self.write_line(1, "capacitive_load_unit(1, pF);")?;
0163:         self.write_line(1, "leakage_power_unit : \"1mW\";")?;
0164:         self.write_line(1, "pulling_resistance_unit :\"1kohm\";")?;
0165:         self.write_line(1, "operating_conditions(OC) {")?;
0166:         self.write_line(2, &format!("process : {};", self.pvt_process()))?;
0167:         self.write_line(2, &format!("voltage : {}", self.pvt_voltage()))?;
0168:         self.write_line(2, &format!("temperature : {}", self.pvt_temp()))?;
0169:         self.write_line(1, "}")?;
0170:         self.write_enter()?;
0171:         Ok(())
0172:     }
0173: 
0174:     fn write_defaults(&mut self) -> YouRAMResult<()> {
0175:         self.write_line(1, &format!("input_threshold_pct_fall      : {};", self.pdk.input_threshold_pct_fall() * 100.0))?;
0176:         self.write_line(1, &format!("input_threshold_pct_fall      : {};", self.pdk.input_threshold_pct_fall() * 100.0))?;
0177:         self.write_line(1, &format!("output_threshold_pct_fall     : {};", self.pdk.output_threshold_pct_fall() * 100.0))?;
0178:         self.write_line(1, &format!("input_threshold_pct_rise      : {};", self.pdk.input_threshold_pct_rise() * 100.0))?;
0179:         self.write_line(1, &format!("output_threshold_pct_rise     : {};", self.pdk.output_threshold_pct_rise() * 100.0))?;
0180:         self.write_line(1, &format!("slew_lower_threshold_pct_fall : {};", self.pdk.slew_lower_threshold_pct_fall() * 100.0))?;
0181:         self.write_line(1, &format!("slew_upper_threshold_pct_fall : {};", self.pdk.slew_upper_threshold_pct_fall() * 100.0))?;
0182:         self.write_line(1, &format!("slew_lower_threshold_pct_rise : {};", self.pdk.slew_lower_threshold_pct_rise() * 100.0))?;
0183:         self.write_line(1, &format!("slew_upper_threshold_pct_rise : {};", self.pdk.slew_upper_threshold_pct_rise() * 100.0))?;
0184:         
0185:         // TODO nom
0186:         self.write_line(1, "default_cell_leakage_power    : 0.0;")?;
0187:         self.write_line(1, "default_leakage_power_density : 0.0;")?;
0188:         self.write_line(1, "default_input_pin_cap         : 1.0;")?;
0189:         self.write_line(1, "default_inout_pin_cap         : 1.0;")?;
0190:         self.write_line(1, "default_output_pin_cap        : 0.0;")?;
0191:         self.write_line(1, "default_max_transition        : 0.5;")?;
0192:         self.write_line(1, "default_fanout_load           : 1.0;")?;
0193:         self.write_line(1, "default_max_fanout            : 4.0;")?;
0194:         self.write_line(1, "default_connection_class      : universal;")?;
0195: 
0196:         self.write_line(1, &format!("voltage_map ({}, {});", Sram::vdd_pn(), self.pvt_voltage()))?;
0197:         self.write_line(1, &format!("voltage_map ({}, 0);", Sram::gnd_pn()))?;
0198:         self.write_line(1, "default_operating_conditions : OC;")?;
0199: 
0200:         self.write_enter()?;
0201: 
0202:         Ok(())
0203:     } 
0204: 
0205:     fn write_luttemplate(&mut self) -> YouRAMResult<()> {
0206:         self.write_line(1, "lu_table_template(CELL_TABLE) {")?;
0207:         self.write_line(2, "variable_1 : input_net_transition;")?;
0208:         self.write_line(2, "variable_2 : total_output_net_capacitance;")?;
0209:         self.write_time_index()?;
0210:         self.write_cap_index()?;
0211:         self.write_line(1, "}")?;
0212:         
0213:         self.write_enter()?;
0214: 
0215:         self.write_line(1, "lu_table_template(CONSTRAINT_TABLE) {")?;
0216:         self.write_line(2, "variable_1 : related_pin_transition;")?;
0217:         self.write_line(2, "variable_2 : constrained_pin_transition;")?;
0218:         self.write_line(1, "}")?;
0219: 
0220:         self.write_enter()?;
0221: 
0222:         Ok(())
0223:     }
0224: 
0225:     fn write_bus(&mut self) -> YouRAMResult<()> {
0226:         self.write_line(1, "type (data) {")?;
0227:         self.write_line(2, "base_type : array;")?;
0228:         self.write_line(2, "data_type : bit;")?;
0229:         self.write_line(2, &format!("bit_width : {};", self.sram.read().word_width()))?;
0230:         self.write_line(2, &format!("bit_from : {};", self.sram.read().word_width() - 1))?;
0231:         self.write_line(2, "bit_to : 0;")?;
0232:         self.write_line(1, "}")?;
0233:         self.write_enter()?;
0234: 
0235:         self.write_line(1, "type (addr) {")?;
0236:         self.write_line(2, "base_type : array;")?;
0237:         self.write_line(2, "data_type : bit;")?;
0238:         self.write_line(2, &format!("bit_width : {};", self.sram.read().address_width()))?;
0239:         self.write_line(2, &format!("bit_from : {};", self.sram.read().address_width() - 1))?;
0240:         self.write_line(2, "bit_to : 0;")?;
0241:         self.write_line(1, "}")?;
0242:         self.write_enter()?;
0243: 
0244:         Ok(())    
0245:     }
0246: 
0247:     fn write_pgpin(&mut self) -> YouRAMResult<()> {
0248:         self.write_line(2, &format!("pg_pin({}) {{", Sram::vdd_pn()))?;
0249:         self.write_line(3, &format!("voltage_name : {};", Sram::vdd_pn()))?;
0250:         self.write_line(3, "pg_type : primary_power;")?;
0251:         self.write_line(2, "}")?;
0252:         self.write_enter()?;
0253: 
0254:         self.write_line(2, &format!("pg_pin({}) {{", Sram::gnd_pn()))?;
0255:         self.write_line(3, &format!("voltage_name : {};", Sram::gnd_pn()))?;
0256:         self.write_line(3, "pg_type : primary_ground;")?;
0257:         self.write_line(2, "}")?;
0258:         self.write_enter()?;
0259:         Ok(())    
0260:     }
0261: 
0262:     fn write_word_bus(&mut self) -> YouRAMResult<()> {
0263:         self.write_word_bus_input()?;
0264:         self.write_word_bus_output()?;
0265:         Ok(())
0266:     }
0267:     
0268:     fn write_word_bus_input(&mut self) -> YouRAMResult<()> {
0269:         // Mark: din?
0270:         self.write_line(2, "bus(din) {")?;
0271:         self.write_line(3, "bus_type  : data;")?;
0272:         self.write_line(3, "direction  : input;")?;
0273:         self.write_line(3, "memory_write() {")?;
0274:         self.write_line(4, "address : addr")?;
0275:         self.write_line(4, "clocked_on  : clk")?;
0276:         self.write_line(3, "}")?; // memory_write
0277:         self.write_line(3, &format!("pin(din[{}:0]) {{", self.sram.read().word_width() - 1))?;
0278:         self.write_dff_timing(4)?;
0279:         self.write_line(3, "}")?; // pin
0280:         self.write_line(2, "}")?; // bus
0281: 
0282:         self.write_enter()?;
0283: 
0284:         Ok(())
0285:     }
0286: 
0287:     fn write_word_bus_output(&mut self) -> YouRAMResult<()> {
0288:         self.write_line(2, "bus(dout) {")?;
0289:         self.write_line(3, "bus_type  : data;")?;
0290:         self.write_line(3, "direction  : output;")?;
0291:         // self.write_line(3, "max_capacitance : ");
0292:         // self.write_line(3, "min_capacitance : ");
0293:         self.write_line(3, "memory_read() {")?;
0294:         self.write_line(4, "address : addr")?;
0295:         self.write_line(3, "}")?; // memory_read()
0296:         // Mark: dout
0297:         self.write_line(3, &format!("pin(dout[{}:0]) {{",  self.sram.read().word_width() - 1))?;
0298:         self.write_timing_charz(4)?;
0299:         self.write_line(3, "}")?; // pin
0300:         self.write_line(2, "}")?; // bus
0301: 
0302:         self.write_enter()?;
0303:         
0304:         Ok(())
0305:     }
0306: 
0307:     fn write_address_bus(&mut self) -> YouRAMResult<()> {
0308:         self.write_line(2, "bus(addr) {")?;
0309:         self.write_line(3, "bus_type  : addr;")?;
0310:         self.write_line(3, "direction  : input;")?;
0311:         // self.write_line(2, "max_capacitance : ");
0312:         // self.write_line(2, "min_capacitance : ");
0313:         self.write_line(3, "memory_read() {")?;
0314:         self.write_line(3, "address : addr")?;
0315:         self.write_line(3, "}")?; // memory_read()
0316:         self.write_line(3, &format!("pin(addr[{}:0]) {{", self.sram.read().address_width()))?;
0317:         self.write_dff_timing(4)?;
0318:         self.write_line(3, "}")?; // pin
0319:         self.write_line(2, "}")?;// bus
0320: 
0321:         self.write_enter()?;
0322: 
0323:         Ok(())
0324:     }
0325: 
0326:     fn write_control_pins(&mut self) -> YouRAMResult<()> {
0327:         self.write_line(2, &format!("pin({}) {{", Sram::chip_sel_bar_pn()))?;
0328:         self.write_line(3, "direction  : input;")?;
0329:         self.write_dff_timing(3)?;
0330:         self.write_line(2, "}")?; // pin
0331:         self.write_enter()?;
0332: 
0333:         self.write_line(2, &format!("pin({}) {{", Sram::write_enable_pn()))?;
0334:         self.write_line(3, "direction  : input;")?;
0335:         self.write_dff_timing(3)?;
0336:         self.write_line(2, "}")?; // pin
0337:         self.write_enter()?;
0338: 
0339:         self.write_line(2, &format!("pin({}) {{", Sram::clock_pn()))?;
0340:         self.write_line(3, "direction  : input;")?;
0341:         self.write_line(2, "}")?; // pin
0342:         self.write_enter()?;
0343: 
0344:         Ok(())
0345:     }
0346: 
0347:     fn write_timing_charz(&mut self, indent: usize) -> YouRAMResult<()> {
0348:         /*
0349:             timing(){ 
0350:                 timing_sense : non_unate; 
0351:                 related_pin : "clk"; 
0352:                 timing_type : falling_edge; 
0353:                 cell_rise(CELL_TABLE) {
0354:                     ...
0355:                 }
0356:                 cell_fall(CELL_TABLE) {
0357:                     ...
0358:                 }
0359:                 rise_transition(CELL_TABLE) {
0360:                     ...
0361:                 }
0362:                 fall_transition(CELL_TABLE) {
0363:                     ...
0364:                 }
0365:             }
0366:         */
0367: 
0368:         self.write_line(indent, "timing() {")?;
0369: 
0370:         self.write_line(indent + 1, "timing_sense : non_unate;")?;
0371:         self.write_line(indent + 1, &format!("related_pin  : \"{}\";", Sram::clock_pn()))?;
0372:         self.write_line(indent + 1, "timing_type : rising_edge;")?;
0373: 
0374:         self.write_line(indent + 1, "cell_rise(CELL_TABLE) {")?;
0375:         self.write_values(indent + 2, &Self::transform_times(&self.delay_lhs))?;
0376:         self.write_line(indent + 1, "}")?; // cell_rise
0377: 
0378:         self.write_line(indent + 1, "cell_fall(CELL_TABLE) {")?;
0379:         self.write_values(indent + 2, &Self::transform_times(&self.delay_hls))?;
0380:         self.write_line(indent + 1, "}")?; // cell_fall
0381: 
0382:         self.write_line(indent + 1, "rise_transition(CELL_TABLE) {")?;
0383:         self.write_values(indent + 2, &Self::transform_times(&self.slew_lhs))?;
0384:         self.write_line(indent + 1, "}")?; // rise_transition
0385: 
0386:         self.write_line(indent + 1, "fall_transition(CELL_TABLE) {")?;
0387:         self.write_values(indent + 2, &Self::transform_times(&self.slew_hls))?;
0388:         self.write_line(indent + 1, "}")?; // fall_transition
0389: 
0390:         self.write_line(indent, "}")?;  // timing
0391: 
0392:         Ok(())
0393:     }
0394: 
0395:     fn write_dff_timing(&mut self, indent: usize) -> YouRAMResult<()> {
0396:         let dff = self.pdk.get_dff(DriveStrength::X1).unwrap();
0397: 
0398:         /*
0399:             timing(){ 
0400:                 timing_type : setup_rising; 
0401:                 related_pin  : "clk"; 
0402:                 rise_constraint(CONSTRAINT_TABLE) {
0403:                     ...
0404:                 fall_constraint(CONSTRAINT_TABLE) {
0405:                     ...
0406:                 }
0407:             }
0408:             timing(){ 
0409:                 timing_type : hold_rising; 
0410:                 related_pin  : "clk0"; 
0411:                 rise_constraint(CONSTRAINT_TABLE) {
0412:                     ...
0413:                 }
0414:                 fall_constraint(CONSTRAINT_TABLE) {
0415:                     ...
0416:                 }
0417:             }
0418:         */
0419:         let setup_rising = &dff.read().setup_rising_timing;
0420:         self.write_line(indent, "timing() {")?;
0421: 
0422:         self.write_line(indent + 1, "timing_type : setup_rising;")?;
0423:         self.write_line(indent + 1, &format!("related_pin  : \"{}\";", Sram::clock_pn()))?;
0424: 
0425:         self.write_line(indent + 1, "rise_constraint(CONSTRAINT_TABLE) {")?;
0426:         self.write_lutable(indent + 2, setup_rising.rise_constraint.as_ref().unwrap())?;
0427:         self.write_line(indent + 1, "}")?;  // rise_constraint
0428: 
0429:         self.write_line(indent + 1, "fall_constraint(CONSTRAINT_TABLE) {")?;
0430:         self.write_lutable(indent + 2, setup_rising.fall_constraint.as_ref().unwrap())?;
0431:         self.write_line(indent + 1, "}")?;  // rise_constraint
0432: 
0433:         self.write_line(indent, "}")?;  // timing
0434:         
0435:         //////////////////////////////////////////////////////////
0436: 
0437:         let hold_rising = &dff.read().hold_rising_timing;
0438:         self.write_line(indent, "timing() {")?;
0439: 
0440:         self.write_line(indent + 1, "timing_type : hold_rising;")?;
0441:         self.write_line(indent + 1, &format!("related_pin  : \"{}\";", Sram::clock_pn()))?;
0442: 
0443:         self.write_line(indent + 1, "rise_constraint(CONSTRAINT_TABLE) {")?;
0444:         self.write_lutable(indent + 2, hold_rising.rise_constraint.as_ref().unwrap())?;
0445:         self.write_line(indent + 1, "}")?;  // rise_constraint
0446: 
0447:         self.write_line(indent + 1, "fall_constraint(CONSTRAINT_TABLE) {")?;
0448:         self.write_lutable(indent + 2, hold_rising.fall_constraint.as_ref().unwrap())?;
0449:         self.write_line(indent + 1, "}")?;  // rise_constraint
0450: 
0451:         self.write_line(indent, "}")?; // timing
0452:         
0453:         Ok(())
0454:     } 
0455: 
0456:     fn write_lutable(&mut self, indent: usize, lutable: &LibLuTable) -> YouRAMResult<()> {
0457:         /*
0458:             index_1: "",
0459:             index_2: "",
0460:             values(\
0461:                 "0.006, 0.008, 0.015",\
0462:                 "0.006, 0.008, 0.015",\
0463:                 "0.006, 0.008, 0.015"\
0464:             );
0465:         */
0466:         self.write_index(indent, 1, lutable.index_1.as_ref().unwrap())?;
0467:         self.write_index(indent, 2, lutable.index_2.as_ref().unwrap())?;
0468:         self.write_values(indent, &lutable.values)?;
0469:         
0470:         Ok(())
0471:     }
0472: 
0473:     fn write_time_index(&mut self) -> YouRAMResult<()> {
0474:         let mut ss = String::new();
0475: 
0476:         write!(ss, "index_1(\"")?;
0477:     
0478:         let mut first_flag = true;
0479:         for v in self.input_net_transitions.iter() {
0480:             if !first_flag {
0481:                 write!(ss, ", ").unwrap();
0482:             } else {
0483:                 first_flag = false;
0484:             }
0485:             write!(ss, "{}", Self::time_value(*v))?;
0486:         }
0487:     
0488:         write!(ss, "\")")?;
0489:         
0490:         self.write_line(2, &ss)?;
0491: 
0492:         Ok(())
0493:     }
0494: 
0495:     fn write_cap_index(&mut self) -> YouRAMResult<()> {
0496:         let mut ss = String::new();
0497: 
0498:         write!(ss, "index_2(\"")?;
0499:     
0500:         let mut first_flag = true;
0501:         for v in self.output_net_capacitances.iter() {
0502:             if !first_flag {
0503:                 write!(ss, ", ").unwrap();
0504:             } else {
0505:                 first_flag = false;
0506:             }
0507:             write!(ss, "{}", Self::cap_value(*v))?;
0508:         }
0509:     
0510:         write!(ss, "\")")?;
0511:         
0512:         self.write_line(2, &ss)?;
0513: 
0514:         Ok(())
0515:     }
0516: 
0517:     fn write_index(&mut self, indent: usize, index: usize,  values: &[f64]) -> YouRAMResult<()> {
0518:         let mut ss = String::new();
0519: 
0520:         write!(ss, "index_{index}(\"")?;
0521:     
0522:         let mut first_flag = true;
0523:         for v in values.iter() {
0524:             if !first_flag {
0525:                 write!(ss, ", ").unwrap();
0526:             } else {
0527:                 first_flag = false;
0528:             }
0529:             write!(ss, "{}", *v)?;
0530:         }
0531:     
0532:         write!(ss, "\")")?;
0533:         
0534:         self.write_line(indent, &ss)?;
0535: 
0536:         Ok(())
0537:     }
0538: 
0539:     fn write_values(&mut self, indent: usize, values: &Vec<Vec<f64>>) -> YouRAMResult<()> {
0540:         /*
0541:             values(\
0542:                 "0.006, 0.008, 0.015",\
0543:                 "0.006, 0.008, 0.015",\
0544:                 "0.006, 0.008, 0.015"\
0545:             );
0546:         */
0547:         self.write_line(indent, "values(")?;
0548:         
0549:         for (i, row) in values.iter().enumerate() {
0550:             let mut line = String::new();
0551:             write!(line, "    \"")?;
0552:             
0553:             for (j, v) in row.iter().enumerate() {
0554:                 if j > 0 {
0555:                     write!(line, ", ")?;
0556:                 }
0557:                 write!(line, "{:.6}", v)?;
0558:             }
0559:             
0560:             write!(line, "\"")?;
0561: 
0562:             if i < values.len() - 1 {
0563:                 write!(line, ",\\")?;
0564:             }
0565:             
0566:             self.write_line(indent + 1, &line)?;
0567:         }
0568: 
0569:         self.write_line(indent, ");")?;
0570: 
0571:         Ok(())
0572:     }
0573: 
0574: }
0575: 
0576: impl LibertyWritor {
0577:     fn write_line(&mut self, indent: usize, s: &str) -> YouRAMResult<()> {
0578:         for _ in 0..indent {
0579:             self.writor.write_all("    ".as_bytes())?;
0580:         }
0581:         self.writor.write_all(s.as_bytes())?;
0582:         self.writor.write_all("\n".as_bytes())?;
0583:         Ok(())
0584:     } 
0585: 
0586:     fn write_enter(&mut self) -> YouRAMResult<()> {
0587:         self.writor.write_all("\n".as_bytes())?;
0588:         Ok(())
0589:     }
0590: }
0591: 
0592: impl LibertyWritor {
0593:     fn transform_times(times: &Vec<Vec<Time>>) -> Vec<Vec<f64>> {
0594:         times.iter().map(|ts| {
0595:             ts.iter().map(|t| Self::time_value(*t)).collect()
0596:         })
0597:         .collect()
0598:     }
0599: 
0600:     fn pvt_process(&self) -> f64 {
0601:         // TODO: save process value?
0602:         match self.pvt.process {
0603:             Process::FastFast => 1.1,
0604:             Process::SlowSlow => 0.9,
0605:             Process::TypeType => 1.0 
0606:         }
0607:     }
0608: 
0609:     fn pvt_voltage(&self) -> f64 {
0610:         Self::voltage_value(self.pvt.voltage)
0611:     }
0612: 
0613:     fn pvt_temp(&self) -> f64 {
0614:         Self::temp_value(self.pvt.temperature)
0615:     }
0616: 
0617:     fn voltage_value(voltage: Voltage) -> f64 {
0618:         // 1v
0619:         voltage.value().to_f64()
0620:     }
0621: 
0622:     fn time_value(time: Time) -> f64 {
0623:         // 1ns
0624:         time.value().to_f64() * 1e9
0625:     }
0626: 
0627:     fn cap_value(cap: Capacitance) -> f64 {
0628:         cap.value().to_f64() * 1e12
0629:     }
0630: 
0631:     fn temp_value(temp: Temperature) -> f64 {
0632:         temp.value().to_f64()
0633:     }
0634: }
0635: 

// File: YouRAM-master\src\export\mod.rs

0001: mod spice;
0002: mod verilog;
0003: mod liberty;
0004: pub use spice::*;
0005: pub use verilog::*;
0006: pub use liberty::*;

// File: YouRAM-master\src\export\spice.rs

0001: use std::collections::HashSet;
0002: use std::fs::File;
0003: use std::io::{BufWriter, Write};
0004: use std::path::Path;
0005: use reda_sp::ToSpice;
0006: use tracing::{debug, info};
0007: use crate::circuit::{CircuitError, ShrCircuit, ShrString};
0008: use crate::YouRAMResult;
0009: 
0010: pub fn write_spice<P: AsRef<Path>, C: Into<ShrCircuit>>(circuit: C, path: P) -> YouRAMResult<()> {
0011:     let circuit = circuit.into();
0012:     let path = path.as_ref();
0013:     info!("write circuit {} to {:?}", circuit.name(), path);
0014:     let file = File::create(path)?;
0015:     let mut writer = BufWriter::new(file);
0016:     let mut exported = HashSet::new();
0017:     write_spice_recursive(&mut writer, &circuit, &mut exported)?;
0018:     Ok(())
0019: }
0020: 
0021: 
0022: fn write_spice_recursive<W: Write>(
0023:     writer: &mut W,
0024:     circuit: &ShrCircuit,
0025:     exported: &mut HashSet<ShrString>,
0026: ) -> YouRAMResult<()> {
0027:     if exported.get(&circuit.name()).is_some() {
0028:         return Ok(());
0029:     }
0030: 
0031:     match circuit {
0032:         ShrCircuit::Module(module) => {
0033:             let module_ref = module.read();
0034:             debug!("write module {}", module_ref.name());
0035:         
0036:             for sub_circuit in module_ref.sub_circuits() {
0037:                 write_spice_recursive(writer, sub_circuit, exported)?;
0038:             }
0039:         
0040:             // .SUBCKT header
0041:             let ports = module_ref.ports();
0042:             let port_names: Vec<_> = ports.iter().map(|p| p.read().name.to_string()).collect();
0043:             writeln!(writer, ".SUBCKT {} {}", module_ref.name(), port_names.join(" "))?;
0044:         
0045:             // instance
0046:             for inst in module_ref.instances() {            
0047:                 let inst = inst.read();
0048:                 
0049:                 let mut pin_nets = Vec::new();
0050:                 for pin in inst.pins.iter() {
0051:                     let pin_ref = pin.read();
0052:                     match &pin_ref.net {
0053:                         // Some(net) => pin_nets.push(format!("{}={}", pin_ref.name, net.read().name)),
0054:                         Some(net) => pin_nets.push(net.read().name.to_string()),
0055:                         None => return Err(CircuitError::InstanceNotConnected(inst.name.to_string()))?,
0056:                     }
0057:                 }
0058:         
0059:                 let subckt_name = inst.template_circuit.name();
0060:                 writeln!(writer, "X{} {} {}", inst.name, pin_nets.join(" "), subckt_name)?;
0061:             }
0062:         
0063:             // connect net
0064:             for (i, (net1, net2)) in module_ref.connected_nets().iter().enumerate() {
0065:                 writeln!(writer, 
0066:                     "Rconnect{} {} {} {}", 
0067:                     i, net1.read().name, net2.read().name, 0.001
0068:                 )?;
0069:             }
0070:         
0071:             writeln!(writer, ".ENDS {}", module_ref.name())?;
0072:         }
0073:         ShrCircuit::Primitive(primitive) => {
0074:             writeln!(writer, "{}", primitive.read().netlist().to_spice())?;
0075:         }
0076:     };
0077: 
0078:     write!(writer, "\n\n")?;
0079:     exported.insert(circuit.name());
0080: 
0081:     Ok(())
0082: }
0083: 

// File: YouRAM-master\src\export\verilog.rs

0001: use std::{fs::File, io::{BufWriter, Write}, path::Path};
0002: use tracing::info;
0003: 
0004: use crate::{circuit::{Shr, Sram}, YouRAMResult};
0005: 
0006: pub fn write_verilog<P: AsRef<Path>>(sram: Shr<Sram>, path: P) -> YouRAMResult<()> {
0007:     let sram_ref = sram.read();
0008:     let path = path.as_ref();
0009: 
0010:     info!("write sram {} to {:?}", sram_ref.name, path);
0011:     let file = File::create(path)?;
0012:     let mut writer = BufWriter::new(file);
0013: 
0014:     writer.write_all("module sram #(\n".as_bytes())?;
0015:     writer.write_all(format!("    parameter ADDR_WIDTH = {},\n", sram_ref.address_width()).as_bytes())?;
0016:     writer.write_all(format!("    parameter DATA_WIDTH = {} \n", sram_ref.word_width()).as_bytes())?;
0017:     writer.write_all(include_str!("./template.v").as_bytes())?;
0018: 
0019:     Ok(())
0020: }

// File: YouRAM-master\src\pdk\cells.rs

0001: use std::collections::HashMap;
0002: use reda_lib::model::{LibCell, LibExpr, LibLibrary, LibPgType, LibPinDirection, LibTimingType};
0003: use reda_sp::Spice;
0004: use crate::{circuit::{Bitcell, ColumnTriGate, Dff, DriveStrength, Leafcell, LogicGate, LogicGateKind, Port, PortDirection, Precharge, SenseAmp, Shr, WriteDriver, BITCELL_NAME, COLUMN_TRI_GATE_NAME, PRECHARGE_NAME, SENSE_AMP_NAME, WRITE_DRIVER_NAME}, ErrorContext, YouRAMResult};
0005: use super::PdkError;
0006: 
0007: pub struct PdkCells {
0008:     pub logicgates: HashMap<(LogicGateKind, DriveStrength), Shr<LogicGate>>,
0009:     pub dffs: HashMap<DriveStrength, Shr<Dff>>,
0010:     pub bitcell: Shr<Leafcell>,
0011:     pub sense_amp: Shr<Leafcell>,
0012:     pub write_driver: Shr<Leafcell>,
0013:     pub column_trigate: Shr<Leafcell>,
0014:     pub precharge: Shr<Leafcell>,
0015: }
0016: 
0017: impl PdkCells {
0018:     pub fn load(library: &LibLibrary, stdcell_spice: &Spice, leafcell_spice: &Spice) -> YouRAMResult<Self> {
0019:         // extract logicgates & dff
0020:         let mut logicgates = HashMap::new();
0021:         let mut dffs = HashMap::new();
0022:         for cell in library.cells.iter() {
0023:             if let Some(dff) = Self::extract_dff(cell, &stdcell_spice).context("extract dff")? {
0024:                 let key = dff.drive_strength;
0025:                 dffs.insert(key, Shr::new(dff));
0026:             } else if let Some(logicgate) = Self::extract_logicgate(cell, &stdcell_spice) {
0027:                 let key = (logicgate.kind, logicgate.drive_strength);
0028:                 logicgates.insert(key, Shr::new(logicgate));
0029:             }
0030:         }
0031: 
0032:         // extract bitcell
0033:         let bitcell 
0034:             = Shr::new(Self::extract_bitcell(&leafcell_spice).context("extract bitcell")?.into());
0035:         let sense_amp
0036:             = Shr::new(Self::extract_sense_amp(&leafcell_spice).context("extract sens_amp")?.into());
0037:         let write_driver
0038:             = Shr::new(Self::extract_write_driver(&leafcell_spice).context("extract write_driver")?.into());
0039:         let column_trigate
0040:             = Shr::new(Self::extract_column_trigate(&leafcell_spice).context("extract column_trigate")?.into());    
0041:         let precharge
0042:             = Shr::new(Self::extract_precharge(&leafcell_spice).context("extract precharge")?.into()); 
0043: 
0044:         Ok(Self {
0045:             logicgates,
0046:             dffs,
0047:             bitcell,
0048:             sense_amp,
0049:             write_driver,
0050:             column_trigate,
0051:             precharge,
0052:         })   
0053:     }
0054: }
0055: 
0056: impl PdkCells {
0057:     pub fn extract_bitcell(spice: &Spice) -> Result<Bitcell, PdkError> {
0058:         let subckt = spice.subckts.iter()
0059:             .find(|s| s.name == BITCELL_NAME)
0060:             .ok_or_else(|| PdkError::UnexitLeafCell(BITCELL_NAME))?
0061:             .clone();
0062: 
0063:         if subckt.ports.len() != 5 {
0064:             return Err(PdkError::UnmatchLeafCellPinSize(5, subckt.ports.len(), BITCELL_NAME));
0065:         }
0066: 
0067:         let bl = Port::new(subckt.ports[0].clone(), PortDirection::InOut);
0068:         let br = Port::new(subckt.ports[1].clone(), PortDirection::InOut);
0069:         let wl = Port::new(subckt.ports[2].clone(), PortDirection::Input);
0070:         let vdd = Port::new(subckt.ports[3].clone(), PortDirection::Vdd);
0071:         let gnd = Port::new(subckt.ports[4].clone(), PortDirection::Gnd);
0072: 
0073:         Ok(Bitcell::new(bl, br, wl, vdd, gnd, subckt))
0074:     }
0075: 
0076:     pub fn extract_sense_amp(spice: &Spice) -> Result<SenseAmp, PdkError> {
0077:         let subckt = spice.subckts.iter()
0078:             .find(|s| s.name == SENSE_AMP_NAME)
0079:             .ok_or_else(|| PdkError::UnexitLeafCell(SENSE_AMP_NAME))?
0080:             .clone();
0081: 
0082:         if subckt.ports.len() != 6 {
0083:             return Err(PdkError::UnmatchLeafCellPinSize(6, subckt.ports.len(), SENSE_AMP_NAME));
0084:         }
0085: 
0086:         let bl   = Port::new(subckt.ports[0].clone(), PortDirection::InOut);
0087:         let br   = Port::new(subckt.ports[1].clone(), PortDirection::InOut);
0088:         let dout = Port::new(subckt.ports[2].clone(), PortDirection::Output);
0089:         let en   = Port::new(subckt.ports[3].clone(), PortDirection::Input);
0090:         let vdd  = Port::new(subckt.ports[4].clone(), PortDirection::Vdd);
0091:         let gnd  = Port::new(subckt.ports[5].clone(), PortDirection::Gnd);
0092: 
0093:         Ok(SenseAmp::new(bl, br, dout, en, vdd, gnd, subckt))
0094:     }
0095: 
0096:     pub fn extract_write_driver(spice: &Spice) -> Result<WriteDriver, PdkError> {
0097:         let subckt = spice.subckts.iter()
0098:             .find(|s| s.name == WRITE_DRIVER_NAME)
0099:             .ok_or_else(|| PdkError::UnexitLeafCell(WRITE_DRIVER_NAME))?
0100:             .clone();
0101: 
0102:         if subckt.ports.len() != 6 {
0103:             return Err(PdkError::UnmatchLeafCellPinSize(6, subckt.ports.len(), WRITE_DRIVER_NAME));
0104:         }
0105: 
0106:         let din = Port::new(subckt.ports[0].clone(), PortDirection::Input);
0107:         let bl  = Port::new(subckt.ports[1].clone(), PortDirection::InOut);
0108:         let br  = Port::new(subckt.ports[2].clone(), PortDirection::InOut);
0109:         let en  = Port::new(subckt.ports[3].clone(), PortDirection::Input);
0110:         let vdd = Port::new(subckt.ports[4].clone(), PortDirection::Vdd);
0111:         let gnd = Port::new(subckt.ports[5].clone(), PortDirection::Gnd);
0112: 
0113:         Ok(WriteDriver::new(din, bl, br, en, vdd, gnd, subckt))
0114:     }
0115: 
0116:     pub fn extract_column_trigate(spice: &Spice) -> Result<ColumnTriGate, PdkError> {
0117:         let subckt = spice.subckts.iter()
0118:             .find(|s| s.name == COLUMN_TRI_GATE_NAME)
0119:             .ok_or_else(|| PdkError::UnexitLeafCell(COLUMN_TRI_GATE_NAME))?
0120:             .clone();
0121: 
0122:         if subckt.ports.len() != 7 {
0123:             return Err(PdkError::UnmatchLeafCellPinSize(7, subckt.ports.len(), COLUMN_TRI_GATE_NAME));
0124:         }
0125: 
0126:         let bl    = Port::new(subckt.ports[0].clone(), PortDirection::InOut);
0127:         let br    = Port::new(subckt.ports[1].clone(), PortDirection::InOut);
0128:         let bl_o  = Port::new(subckt.ports[2].clone(), PortDirection::InOut);
0129:         let br_o  = Port::new(subckt.ports[3].clone(), PortDirection::InOut);
0130:         let sel   = Port::new(subckt.ports[4].clone(), PortDirection::Input);
0131:         let vdd   = Port::new(subckt.ports[5].clone(), PortDirection::Vdd);
0132:         let gnd   = Port::new(subckt.ports[6].clone(), PortDirection::Gnd);
0133: 
0134:         Ok(ColumnTriGate::new(bl, br, bl_o, br_o, sel, vdd, gnd, subckt))
0135:     }
0136: 
0137:     pub fn extract_precharge(spice: &Spice) -> Result<Precharge, PdkError> {
0138:         let subckt = spice.subckts.iter()
0139:             .find(|s| s.name == PRECHARGE_NAME)
0140:             .ok_or_else(|| PdkError::UnexitLeafCell(PRECHARGE_NAME))?
0141:             .clone();
0142: 
0143:         if subckt.ports.len() != 4 {
0144:             return Err(PdkError::UnmatchLeafCellPinSize(4, subckt.ports.len(), PRECHARGE_NAME));
0145:         }
0146: 
0147:         let bl    = Port::new(subckt.ports[0].clone(), PortDirection::InOut);
0148:         let br    = Port::new(subckt.ports[1].clone(), PortDirection::InOut);
0149:         let enable   = Port::new(subckt.ports[2].clone(), PortDirection::Input);
0150:         let vdd   = Port::new(subckt.ports[3].clone(), PortDirection::Vdd);
0151: 
0152:         Ok(Precharge::new(bl, br, enable, vdd, subckt))   
0153:     }
0154: }
0155: 
0156: impl PdkCells {
0157:     pub fn extract_dff(cell: &LibCell, spice: &Spice) -> Result<Option<Dff>, PdkError> {
0158:         // ff exit?
0159:         let ff = match cell.ff.as_ref() {
0160:             Some(ff) => ff,
0161:             None => return Ok(None),
0162:         };
0163: 
0164:         // TODO: better way to check (din, clk, q, qn)
0165:         if cell.input_pins().count() == 2 && cell.output_pins().count() == 2 {
0166:             let subckt = spice.subckts.iter()
0167:                 .find(|s| s.name == cell.name)
0168:                 .ok_or_else(|| PdkError::CellNotFoundInSpiceFile(cell.name.to_string()))?
0169:                 .clone();
0170: 
0171:             let drive_strength = DriveStrength::try_from_cell(cell)
0172:                 .ok_or_else(|| PdkError::CanNotGetDriverStrenghtInCell(cell.name.to_string()))?;
0173: 
0174:             let mut ports = vec![];
0175:             let mut din_port_index = None;
0176:             let mut clk_port_index = None;
0177:             let mut q_port_index = None;
0178:             let mut qn_port_index = None;
0179:             let mut vdd_port_index = None;
0180:             let mut gnd_port_index = None;
0181:             let mut hold_rising_timing = None;
0182:             let mut setup_rising_timing = None;
0183: 
0184:             for (port_index, port_name) in subckt.ports.iter().enumerate() {
0185:                 if let Some(pin) = cell.get_pin(&port_name) {
0186:                     let direction = match pin.direction {
0187:                         LibPinDirection::Input => {
0188:                             if pin.clock == Some(true) {
0189:                                 clk_port_index = Some(port_index);
0190:                             } else {
0191:                                 din_port_index = Some(port_index);
0192:                                 // find input pin , get setup and hold 
0193:                                 for timing in pin.timings.iter() {
0194:                                     // TODO: Rising / Falling
0195:                                     match timing.timing_type {
0196:                                         Some(LibTimingType::HoldRising) => {
0197:                                             hold_rising_timing = Some(timing.clone());
0198:                                         }
0199:                                         Some(LibTimingType::SetupRising) => {
0200:                                             setup_rising_timing = Some(timing.clone());
0201:                                         }
0202:                                         _ => {
0203:                                         }
0204:                                     }
0205:                                 }
0206:                             }
0207:                             PortDirection::Input
0208:                         }
0209:                         LibPinDirection::Output => {
0210:                             let function = pin.function.as_ref().ok_or_else(|| PdkError::ExpectAttrButNotFound("function"))?;
0211:                             if let LibExpr::Var(name) = function {
0212:                                 if name.as_str() == ff.names[0].as_str() {
0213:                                     q_port_index = Some(port_index);
0214:                                 } else if name.as_str() == ff.names[1].as_str() {
0215:                                     qn_port_index = Some(port_index);
0216:                                 } else {
0217:                                     panic!()
0218:                                 }
0219:                             } else {
0220:                                 panic!();
0221:                             }
0222:                             PortDirection::Output
0223:                         }
0224:                         _ => panic!(),
0225:                     };
0226:                     ports.push(Port::new(port_name.clone(), direction));
0227:                 } else if let Some(pg_pin) = cell.get_pg_pin(&port_name) {
0228:                     match pg_pin.pg_type {
0229:                         LibPgType::PrimaryPower => {
0230:                             vdd_port_index = Some(port_index);
0231:                             ports.push(Port::new(port_name.clone(), PortDirection::Vdd));
0232:                         }
0233:                         LibPgType::PrimaryGround => {
0234:                             gnd_port_index = Some(port_index);
0235:                             ports.push(Port::new(port_name.clone(), PortDirection::Gnd));
0236:                         }
0237:                         _ => return Ok(None),
0238:                     }
0239:                 }
0240:             }
0241: 
0242:             Ok(Some(Dff {
0243:                 name: cell.name.clone().into(),
0244:                 drive_strength,
0245:                 ports,
0246:                 din_port_index: din_port_index.ok_or_else(|| PdkError::LackPort("din"))?,
0247:                 clk_port_index: clk_port_index.ok_or_else(|| PdkError::LackPort("clk"))?,
0248:                 q_port_index: q_port_index.ok_or_else(|| PdkError::LackPort("q"))?,
0249:                 qn_port_index: qn_port_index.ok_or_else(|| PdkError::LackPort("qn"))?,
0250:                 vdd_port_index: vdd_port_index.ok_or_else(|| PdkError::LackPort("vdd"))?,
0251:                 gnd_port_index: gnd_port_index.ok_or_else(|| PdkError::LackPort("gnd"))?,
0252:                 hold_rising_timing: hold_rising_timing.ok_or_else(|| PdkError::ExpectAttrButNotFound("setup_rising_timing"))?,
0253:                 setup_rising_timing: setup_rising_timing.ok_or_else(|| PdkError::ExpectAttrButNotFound("setup_rising_timing"))?,
0254:                 netlist: subckt,
0255:             }))
0256:         } else {
0257:             Ok(None)
0258:         }
0259:     }
0260: 
0261:     // TODO: better way to extract logicgate!(add result)
0262:     pub fn extract_logicgate(cell: &LibCell, spice: &Spice) -> Option<LogicGate> {
0263:         // 1. 鏍规嵁杈撳嚭 pin function 鍒ゆ柇绫诲瀷
0264:         if cell.output_pins().count() != 1 {
0265:             return None;
0266:         }
0267:         let output_pin = cell.output_pins().nth(0)?;
0268:         let function = output_pin.function.as_ref()?;
0269:         let kind = Self::try_transform_expr(function)?;
0270: 
0271:         // 2. 鏌ユ壘 SPICE subckt
0272:         let subckt = spice.subckts.iter()
0273:             .find(|s| s.name == cell.name)?
0274:             .clone();
0275: 
0276:         // 3. 鏋勫缓绔彛鍒楄〃
0277:         let mut ports = vec![];
0278:         let mut input_port_indexs = vec![];
0279:         let mut output_port_index = None;
0280:         let mut vdd_port_index = None;
0281:         let mut gnd_port_index = None;
0282: 
0283:         for (port_index, port_name) in subckt.ports.iter().enumerate() {
0284:             if let Some(pin) = cell.get_pin(&port_name) {
0285:                 let direction = match pin.direction {
0286:                     LibPinDirection::Input => {
0287:                         input_port_indexs.push(port_index);
0288:                         PortDirection::Input
0289:                     }
0290:                     LibPinDirection::Output => {
0291:                         output_port_index = Some(port_index);
0292:                         PortDirection::Output
0293:                     }
0294:                     _ => panic!(),
0295:                 };
0296:                 ports.push(Port::new(port_name.clone(), direction));
0297:             } else if let Some(pg_pin) = cell.get_pg_pin(&port_name) {
0298:                 match pg_pin.pg_type {
0299:                     LibPgType::PrimaryPower => {
0300:                         vdd_port_index = Some(port_index);
0301:                         ports.push(Port::new(port_name.clone(), PortDirection::Vdd));
0302:                     }
0303:                     LibPgType::PrimaryGround => {
0304:                         gnd_port_index = Some(port_index);
0305:                         ports.push(Port::new(port_name.clone(), PortDirection::Gnd));
0306:                     }
0307:                     _ => return None,
0308:                 }
0309:             }
0310:         }
0311: 
0312:         if kind == LogicGateKind::Inv && ports.len() != 4 {
0313:             return None;
0314:         }
0315: 
0316:         let drive_strength = DriveStrength::try_from_cell(cell)?;
0317: 
0318:         Some(LogicGate {
0319:             name: cell.name.clone().into(),
0320:             drive_strength,
0321:             kind,
0322:             ports,
0323:             input_port_indexs,
0324:             output_port_index: output_port_index?,
0325:             vdd_port_index: vdd_port_index?,
0326:             gnd_port_index: gnd_port_index?,
0327:             netlist: subckt,
0328:         })
0329:     }
0330: 
0331:     pub fn try_transform_expr(expr: &LibExpr) -> Option<LogicGateKind> {
0332:         match expr {
0333:             LibExpr::Not(inner) => match &**inner {
0334:                 LibExpr::Var(_) => Some(LogicGateKind::Inv),
0335:                 LibExpr::And(_) => Self::analyze_and_or(inner, true).map(LogicGateKind::Nand),
0336:                 LibExpr::Or(_) => Self::analyze_and_or(inner, true).map(LogicGateKind::Nor),
0337:                 _ => None,
0338:             },
0339:             LibExpr::And(_) => Self::analyze_and_or(expr, false).map(LogicGateKind::And),
0340:             LibExpr::Or(_) => Self::analyze_and_or(expr, false).map(LogicGateKind::Or),
0341:             _ => None,
0342:         }
0343:     }
0344: 
0345:     fn analyze_and_or(expr: &LibExpr, ignore_not: bool) -> Option<usize> {
0346:         match expr {
0347:             LibExpr::And(children) => {
0348:                 let mut count = 0;
0349:                 for c in children {
0350:                     match c {
0351:                         LibExpr::Var(_) | LibExpr::Const(_) => count += 1,
0352:                         LibExpr::Not(inner) if ignore_not => {
0353:                             if matches!(**inner, LibExpr::Var(_) | LibExpr::Const(_)) {
0354:                                 count += 1;
0355:                             } else {
0356:                                 return None;
0357:                             }
0358:                         }
0359:                         LibExpr::And(_) => count += Self::analyze_and_or(c, ignore_not)?,
0360:                         _ => return None, // 鍑虹幇 Or銆乆or 绛夋贩鍚堥€昏緫 鈫?涓嶆槸绾?AND 鏍?                    }
0361:                 }
0362:                 Some(count)
0363:             }
0364:             LibExpr::Or(children) => {
0365:                 let mut count = 0;
0366:                 for c in children {
0367:                     match c {
0368:                         LibExpr::Var(_) | LibExpr::Const(_) => count += 1,
0369:                         LibExpr::Not(inner) if ignore_not => {
0370:                             if matches!(**inner, LibExpr::Var(_) | LibExpr::Const(_)) {
0371:                                 count += 1;
0372:                             } else {
0373:                                 return None;
0374:                             }
0375:                         }
0376:                         LibExpr::Or(_) => count += Self::analyze_and_or(c, ignore_not)?,
0377:                         _ => return None, // 鍑虹幇 And銆乆or 绛夋贩鍚堥€昏緫 鈫?涓嶆槸绾?OR 鏍?                    }
0378:                 }
0379:                 Some(count)
0380:             }
0381:             _ => None,
0382:         }
0383:     }
0384: }
0385: 
0386: #[cfg(test)]
0387: mod tests {
0388:     use std::str::FromStr;
0389:     use super::*;
0390: 
0391:     fn str_to_kind(s: &str) -> Option<LogicGateKind> {
0392:         let expr = LibExpr::from_str(s).unwrap();
0393:         PdkCells::try_transform_expr(&expr)
0394:     }
0395: 
0396:     #[test]
0397:     fn test_std_cell_kind() {
0398: 
0399:         // 鍗曞彉閲?NOT -> Inv
0400:         assert_eq!(str_to_kind("!A").unwrap(), LogicGateKind::Inv);
0401: 
0402:         // 绠€鍗?AND
0403:         assert_eq!(str_to_kind("(A1 & A2)").unwrap(), LogicGateKind::And(2));
0404: 
0405:         // 宓屽 AND
0406:         assert_eq!(str_to_kind("((A1 & A2) & A3)").unwrap(), LogicGateKind::And(3));
0407:         assert_eq!(str_to_kind("(((A1 & A2) & A3) & A4)").unwrap(), LogicGateKind::And(4));
0408: 
0409:         // 绠€鍗?NAND
0410:         assert_eq!(str_to_kind("!(A1 & A2)").unwrap(), LogicGateKind::Nand(2));
0411: 
0412:         // 宓屽 NAND
0413:         assert_eq!(str_to_kind("!((A1 & A2) & A3)").unwrap(), LogicGateKind::Nand(3));
0414:         assert_eq!(str_to_kind("!(((A1 & A2) & A3) & A4)").unwrap(), LogicGateKind::Nand(4));
0415: 
0416:         // 绠€鍗?OR
0417:         assert_eq!(str_to_kind("(A1 | A2)").unwrap(), LogicGateKind::Or(2));
0418: 
0419:         // 绠€鍗?NOR
0420:         assert_eq!(str_to_kind("!(A1 | A2)").unwrap(), LogicGateKind::Nor(2));
0421: 
0422:         // 宓屽 OR
0423:         assert_eq!(str_to_kind("((A1 | A2) | A3)").unwrap(), LogicGateKind::Or(3));
0424: 
0425:         // 宓屽 NOR
0426:         assert_eq!(str_to_kind("!((A1 | A2) | A3)").unwrap(), LogicGateKind::Nor(3));
0427:     }
0428: }

// File: YouRAM-master\src\pdk\config.rs

0001: use std::{collections::HashMap, path::{Path, PathBuf}};
0002: use serde::{Deserialize, Serialize};
0003: use crate::{ErrorContext, YouRAMResult};
0004: use super::Process;
0005: 
0006: pub const PDK_CONFIG: &'static str = "config.json";
0007: 
0008: #[derive(Debug, Serialize, Deserialize)]
0009: pub struct PdkConfig {
0010:     #[serde(skip)]
0011:     pub pdk_path: PathBuf,
0012: 
0013:     pub stdcell_liberty: PathBuf,
0014:     pub stdcell_spice: PathBuf,
0015:     pub leafcell_spice: PathBuf,
0016:     pub models: HashMap<Process, PdkModelPath>,
0017: }
0018: 
0019: #[derive(Debug, Serialize, Deserialize)]
0020: pub struct PdkModelPath {
0021:     pub nmos: PathBuf,
0022:     pub pmos: PathBuf, 
0023: }
0024: 
0025: impl PdkConfig {
0026:     pub fn load<P: AsRef<Path>>(pdk_path: P) -> YouRAMResult<Self> {
0027:         let pdk_path: &Path = pdk_path.as_ref();
0028:         let config_path = pdk_path.join(PDK_CONFIG);
0029:         let config_content = std::fs::read_to_string(config_path).context("read pdk config")?;
0030:         let mut config: PdkConfig = serde_json::from_str(&config_content).context("parse pdk config")?;
0031:         config.pdk_path = pdk_path.into();
0032: 
0033:         Ok(config)
0034:     }
0035: 
0036:     pub fn nmos_model_path(&self, process: Process) -> Option<PathBuf> {
0037:         let models = self.models.get(&process)?;
0038:         Some(self.json_to_pdk(&models.nmos))
0039:     }
0040: 
0041:     pub fn pmos_model_path(&self, process: Process) -> Option<PathBuf> {
0042:         let models = self.models.get(&process)?;
0043:         Some(self.json_to_pdk(&models.pmos))
0044:     }
0045: 
0046: 
0047:     pub fn stdcell_liberty_path(&self) -> PathBuf {
0048:         self.json_to_pdk(&self.stdcell_liberty)
0049:     }
0050: 
0051:     pub fn stdcell_spice_path(&self) -> PathBuf {
0052:         self.json_to_pdk(&self.stdcell_spice)
0053:     }
0054: 
0055:     pub fn leafcell_spice_path(&self) -> PathBuf {
0056:         self.json_to_pdk(&self.leafcell_spice)
0057:     }
0058: 
0059:     #[inline]
0060:     fn json_to_pdk(&self, sub_path: impl AsRef<Path>) -> PathBuf {
0061:         self.pdk_path.join(sub_path.as_ref())
0062:     }
0063: }

// File: YouRAM-master\src\pdk\error.rs

0001: use reda_lib::error::LibError;
0002: 
0003: use super::Process;
0004: 
0005: #[derive(Debug, thiserror::Error)]
0006: pub enum PdkError {
0007:     #[error("un exit leaf cell '{0}'")]
0008:     UnexitLeafCell(&'static str),
0009: 
0010:     #[error("expect {0} pins but got {1} in leaf cell '{2}'")]
0011:     UnmatchLeafCellPinSize(usize, usize, &'static str),
0012: 
0013:     #[error("nmos model in process {0} not found")]
0014:     NmosModelNotFound(Process),
0015: 
0016:     #[error("default operating conditions '{0}' not found")]
0017:     DefaultOperatingConditionsNotFound(String),
0018: 
0019:     #[error("operating conditions not found in library")]
0020:     OperatingConditionsNotFound,
0021: 
0022:     #[error("unkonw pg pin name {0}")]
0023:     UnkownPgPinName(String),
0024: 
0025:     #[error("cell {0} not found in spice file not exit in lib file")]
0026:     CellNotFoundInSpiceFile(String),
0027: 
0028:     #[error("can't get driver strenght in cell {0}")]
0029:     CanNotGetDriverStrenghtInCell(String),
0030: 
0031:     #[error("lack port {0}")]
0032:     LackPort(&'static str),
0033: 
0034:     #[error("expect attr {0} but no exit")]
0035:     ExpectAttrButNotFound(&'static str),
0036: 
0037:     #[error(transparent)]
0038:     Liberty(#[from] LibError),
0039: }

// File: YouRAM-master\src\pdk\information.rs

0001: use reda_lib::model::{LibLibrary, LibOperatingConditions, LibPinDirection};
0002: use reda_unit::{Capacitance, Temperature, Time, Voltage};
0003: use crate::{circuit::{DriveStrength, LogicGateKind}, ErrorContext, YouRAMResult};
0004: use super::{cells::PdkCells, PdkError, Process, Pvt};
0005: 
0006: #[derive(Debug, Clone)]
0007: pub struct PdkInformation {
0008:     pub name: String,
0009: 
0010:     pub pvt: Pvt,
0011: 
0012:     pub nom_process: Option<f64>,
0013:     pub nom_temperature: Option<Temperature>,
0014:     pub nom_voltage: Option<Voltage>,
0015: 
0016:     pub default_inout_pin_cap: Option<Capacitance>,
0017:     pub default_input_pin_cap: Option<Capacitance>,
0018:     pub default_output_pin_cap: Option<Capacitance>,
0019:     pub default_fanout_load: Option<Capacitance>,
0020:     pub default_max_transition: Option<Time>,
0021: 
0022:     pub slew_lower_threshold_pct_fall: f64,
0023:     pub slew_lower_threshold_pct_rise: f64,
0024:     pub slew_upper_threshold_pct_fall: f64,
0025:     pub slew_upper_threshold_pct_rise: f64,
0026: 
0027:     pub input_threshold_pct_fall: f64,
0028:     pub input_threshold_pct_rise: f64,
0029:     pub output_threshold_pct_fall: f64,
0030:     pub output_threshold_pct_rise: f64,
0031: 
0032:     pub timing_input_net_transitions: Vec<Time>,
0033:     pub timing_output_net_capacitances: Vec<Capacitance>,
0034: }
0035: 
0036: impl PdkInformation {
0037:     pub fn load(library: &LibLibrary, cells: &PdkCells) -> YouRAMResult<Self> {
0038:         let time_unit = library.time_unit;
0039:         let capacitance_unit = library.capacitive_load_unit.unwrap_or_default();
0040: 
0041:         let nom_process = library.nom_process;
0042:         let nom_temperature = library.nom_temperature.map(Temperature::from);
0043:         let nom_voltage = library.nom_voltage.map(Voltage::from);
0044: 
0045:         let default_inout_pin_cap = library.default_inout_pin_cap
0046:             .map(|value| Capacitance::from(capacitance_unit.value() * value) );
0047:         let default_input_pin_cap = library.default_input_pin_cap
0048:             .map(|value| Capacitance::from(capacitance_unit.value() * value) );
0049:         let default_output_pin_cap = library.default_output_pin_cap
0050:             .map(|value| Capacitance::from(capacitance_unit.value() * value) );
0051:         let default_fanout_load = library.default_fanout_load
0052:             .map(|value| Capacitance::from(capacitance_unit.value() * value) );
0053:         let default_max_transition = library.default_max_transition
0054:             .map(|value| Time::from(time_unit.value() * value) );
0055: 
0056:         let pvt = Self::extract_pvt(library).context("extract pvt")?;
0057: 
0058:         let (timing_input_net_transitions, timing_output_net_capacitances) = Self::extract_timings(library, cells)?;
0059: 
0060:         Ok(Self {
0061:             name: library.name.clone(),
0062:             pvt,
0063:             nom_process,
0064:             nom_temperature,
0065:             nom_voltage,
0066:             default_inout_pin_cap,
0067:             default_input_pin_cap,
0068:             default_output_pin_cap,
0069:             default_fanout_load,
0070:             default_max_transition,
0071:             slew_lower_threshold_pct_fall: library.slew_lower_threshold_pct_fall / 100.0,
0072:             slew_lower_threshold_pct_rise: library.slew_lower_threshold_pct_rise / 100.0,
0073:             slew_upper_threshold_pct_fall: library.slew_upper_threshold_pct_fall / 100.0,
0074:             slew_upper_threshold_pct_rise: library.slew_upper_threshold_pct_rise / 100.0,
0075:             input_threshold_pct_fall: library.input_threshold_pct_fall / 100.0,
0076:             input_threshold_pct_rise: library.input_threshold_pct_rise / 100.0,
0077:             output_threshold_pct_fall: library.output_threshold_pct_fall / 100.0,
0078:             output_threshold_pct_rise: library.output_threshold_pct_rise / 100.0,
0079:             timing_input_net_transitions,
0080:             timing_output_net_capacitances
0081:         })
0082:         
0083:     }
0084: 
0085:     fn extract_timings(library: &LibLibrary, cells: &PdkCells) -> YouRAMResult<(Vec<Time>, Vec<Capacitance>)> {
0086:         let time_unit = library.time_unit;
0087:         let capacitance_unit = library.capacitive_load_unit.unwrap_or_default();
0088:         
0089:         // MARK: use inv1 cell's timing info
0090:         let inv_x1_name = cells.logicgates.get(&(LogicGateKind::Inv, DriveStrength::X1)).unwrap().read().name.to_string();
0091:         let cell = library.cell(&inv_x1_name).unwrap();
0092:         for pin in cell.pins.iter() {
0093:             if let LibPinDirection::Output = pin.direction {
0094:                 let timing = &pin.timings[0];
0095:                 let cell_fall = timing.cell_fall.as_ref().unwrap();
0096:                 let input_net_transitions = cell_fall.index_1.as_ref().unwrap();
0097:                 let total_output_net_capacitances = cell_fall.index_2.as_ref().unwrap();
0098:                 
0099:                 let timing_input_net_transitions = input_net_transitions.iter()
0100:                     .map(|v| Time::from(time_unit.value() * v))
0101:                     .collect();
0102:                 let timing_output_net_capacitances = total_output_net_capacitances.iter()
0103:                     .map(|v| Capacitance::from(capacitance_unit.value() * v))
0104:                     .collect();
0105: 
0106:                 return Ok((timing_input_net_transitions, timing_output_net_capacitances));
0107:             }
0108:         }
0109:         unreachable!()
0110:     }
0111: 
0112:     fn extract_pvt(library: &LibLibrary) -> YouRAMResult<Pvt> {
0113:         let voltage_unit = library.voltage_unit;
0114: 
0115:         let transform_to_pvt = |oc: &LibOperatingConditions| {
0116:             let process = match oc.process {
0117:                 1.0 => Process::TypeType,
0118:                 p if p > 1.0 => Process::FastFast,
0119:                 _ => Process::SlowSlow,
0120:             };
0121:             Pvt::new(process, oc.voltage * voltage_unit.value(), oc.temperature)
0122:         };
0123: 
0124:         match library.default_operating_conditions.as_ref() {
0125:             Some(default_operating_conditions) => {
0126:                 for oc in library.operating_conditions.iter() {
0127:                     if oc.name.as_str() == default_operating_conditions {
0128:                         return Ok(transform_to_pvt(oc));
0129:                     }
0130:                 }
0131: 
0132:                 Err(PdkError::DefaultOperatingConditionsNotFound(default_operating_conditions.to_string()))?
0133:             }
0134:             None => {
0135:                 // Find first operating_conditions
0136:                 let oc = library.operating_conditions
0137:                     .first()
0138:                     .ok_or_else(|| PdkError::OperatingConditionsNotFound)?;
0139: 
0140:                 Ok(transform_to_pvt(oc))
0141:             }
0142:         }
0143:     }
0144: }

// File: YouRAM-master\src\pdk\mod.rs

0001: mod error;
0002: mod cells;
0003: mod types;
0004: mod config;
0005: mod information;
0006: use cells::PdkCells;
0007: pub use error::*;
0008: use information::PdkInformation;
0009: use reda_unit::{Capacitance, Temperature, Time, Voltage};
0010: pub use types::*;
0011: pub use config::*;
0012: 
0013: use std::path::{Path, PathBuf};
0014: use reda_lib::model::LibLibrary;
0015: use reda_sp::Spice;
0016: use crate::{circuit::{Dff, DriveStrength, Leafcell, LogicGate, LogicGateKind, Shr}, ErrorContext, YouRAMError, YouRAMResult};
0017: 
0018: pub struct Pdk {
0019:     config: PdkConfig,
0020:     infomation: PdkInformation,
0021:     cells: PdkCells,
0022: }
0023: 
0024: // Interface for config
0025: impl Pdk {
0026:     pub fn nmos_model_path(&self, process: Process) -> Result<PathBuf, PdkError> {
0027:         self.config.nmos_model_path(process)
0028:             .ok_or_else(|| PdkError::NmosModelNotFound(process))
0029:     }
0030: 
0031:     pub fn pmos_model_path(&self, process: Process) -> Result<PathBuf, PdkError> {
0032:         self.config.pmos_model_path(process)
0033:             .ok_or_else(|| PdkError::NmosModelNotFound(process))
0034:     }
0035: 
0036:     #[inline]
0037:     pub fn pdk_root_path(&self) -> &Path {
0038:         &self.config.pdk_path
0039:     }
0040: 
0041:     #[inline]
0042:     pub fn stdcell_liberty_path(&self) -> PathBuf {
0043:         self.config.stdcell_liberty_path()
0044:     }
0045: 
0046:     #[inline]
0047:     pub fn stdcell_spice_path(&self) -> PathBuf {
0048:         self.config.stdcell_spice_path()
0049:     }
0050: 
0051:     #[inline]
0052:     pub fn leafcell_spice_path(&self) -> PathBuf {
0053:         self.config.leafcell_spice_path()
0054:     }
0055: }
0056: 
0057: // Interface for infomation
0058: impl Pdk {
0059:     #[inline]
0060:     pub fn name(&self) -> &str {
0061:         &self.infomation.name
0062:     }
0063: 
0064:     #[inline]
0065:     pub fn pvt(&self) -> &Pvt {
0066:         &self.infomation.pvt
0067:     }
0068: 
0069:     #[inline]
0070:     pub fn nom_process(&self) -> Option<f64> {
0071:         self.infomation.nom_process
0072:     }
0073: 
0074:     #[inline]
0075:     pub fn nom_temperature(&self) -> Option<Temperature> {
0076:         self.infomation.nom_temperature
0077:     }
0078: 
0079:     #[inline]
0080:     pub fn nom_voltage(&self) -> Option<Voltage> {
0081:         self.infomation.nom_voltage
0082:     }
0083: 
0084:     #[inline]
0085:     pub fn default_inout_pin_cap(&self) -> Option<Capacitance> {
0086:         self.infomation.default_inout_pin_cap
0087:     }
0088: 
0089:     #[inline]
0090:     pub fn default_input_pin_cap(&self) -> Option<Capacitance> {
0091:         self.infomation.default_input_pin_cap
0092:     }
0093: 
0094:     #[inline]
0095:     pub fn default_output_pin_cap(&self) -> Option<Capacitance> {
0096:         self.infomation.default_output_pin_cap
0097:     }
0098: 
0099:     #[inline]
0100:     pub fn default_fanout_load(&self) -> Option<Capacitance> {
0101:         self.infomation.default_fanout_load
0102:     }
0103: 
0104:     #[inline]
0105:     pub fn default_max_transition(&self) -> Option<Time> {
0106:         self.infomation.default_max_transition
0107:     }
0108: 
0109:     #[inline]
0110:     pub fn slew_lower_threshold_pct_fall(&self) -> f64 {
0111:         self.infomation.slew_lower_threshold_pct_fall
0112:     }
0113: 
0114:     #[inline]
0115:     pub fn slew_lower_threshold_pct_rise(&self) -> f64 {
0116:         self.infomation.slew_lower_threshold_pct_rise
0117:     }
0118: 
0119:     #[inline]
0120:     pub fn slew_upper_threshold_pct_rise(&self) -> f64 {
0121:         self.infomation.slew_upper_threshold_pct_rise
0122:     }
0123: 
0124:     #[inline]
0125:     pub fn slew_upper_threshold_pct_fall(&self) -> f64 {
0126:         self.infomation.slew_upper_threshold_pct_fall
0127:     }
0128: 
0129:     #[inline]
0130:     pub fn input_threshold_pct_fall(&self) -> f64 {
0131:         self.infomation.input_threshold_pct_fall
0132:     }
0133: 
0134:     #[inline]
0135:     pub fn input_threshold_pct_rise(&self) -> f64 {
0136:         self.infomation.input_threshold_pct_rise
0137:     }
0138: 
0139:     #[inline]
0140:     pub fn output_threshold_pct_fall(&self) -> f64 {
0141:         self.infomation.output_threshold_pct_fall
0142:     }
0143: 
0144:     #[inline]
0145:     pub fn output_threshold_pct_rise(&self) -> f64 {
0146:         self.infomation.output_threshold_pct_rise
0147:     }
0148: 
0149:     #[inline]
0150:     pub fn timing_input_net_transitions(&self) -> &[Time] {
0151:         &self.infomation.timing_input_net_transitions
0152:     }
0153: 
0154:     #[inline]
0155:     pub fn timing_output_net_capacitances(&self) -> &[Capacitance] {
0156:         &self.infomation.timing_output_net_capacitances
0157:     }
0158: }
0159: 
0160: // Interface for cells
0161: impl Pdk {
0162:     #[inline]
0163:     pub fn get_logicgate(&self, kind: LogicGateKind, drive_strength: DriveStrength) -> Option<Shr<LogicGate>> {
0164:         self.cells.logicgates.get(&(kind, drive_strength)).cloned()
0165:     }
0166: 
0167:     pub fn get_and(&self, input_size: usize, drive_strength: DriveStrength) -> Option<Shr<LogicGate>> {
0168:         let kind = LogicGateKind::And(input_size);
0169:         self.get_logicgate(kind, drive_strength)
0170:     }
0171: 
0172:     pub fn get_nand(&self, input_size: usize, drive_strength: DriveStrength) -> Option<Shr<LogicGate>> {
0173:         let kind = LogicGateKind::Nand(input_size);
0174:         self.get_logicgate(kind, drive_strength)
0175:     }
0176: 
0177:     pub fn get_or(&self, input_size: usize, drive_strength: DriveStrength) -> Option<Shr<LogicGate>> {
0178:         let kind = LogicGateKind::Or(input_size);
0179:         self.get_logicgate(kind, drive_strength)
0180:     }
0181: 
0182:     pub fn get_nor(&self, input_size: usize, drive_strength: DriveStrength) -> Option<Shr<LogicGate>> {
0183:         let kind = LogicGateKind::Nor(input_size);
0184:         self.get_logicgate(kind, drive_strength)
0185:     }
0186: 
0187:     pub fn get_inv(&self, drive_strength: DriveStrength) -> Option<Shr<LogicGate>> {
0188:         let kind = LogicGateKind::Inv;
0189:         self.get_logicgate(kind, drive_strength)
0190:     }
0191: 
0192:     #[inline]
0193:     pub fn get_dff(&self, drive_strength: DriveStrength) -> Option<Shr<Dff>> {
0194:         self.cells.dffs.get(&drive_strength).cloned()
0195:     }
0196: 
0197:     #[inline]
0198:     pub fn get_bitcell(&self) -> Shr<Leafcell> {
0199:         self.cells.bitcell.clone()
0200:     }
0201: 
0202:     #[inline]
0203:     pub fn get_sense_amp(&self) -> Shr<Leafcell> {
0204:         self.cells.sense_amp.clone()
0205:     }
0206: 
0207:     #[inline]
0208:     pub fn get_write_driver(&self) -> Shr<Leafcell> {
0209:         self.cells.write_driver.clone()
0210:     }
0211: 
0212:     #[inline]
0213:     pub fn get_column_trigate(&self) -> Shr<Leafcell> {
0214:         self.cells.column_trigate.clone()
0215:     }
0216: 
0217:     #[inline]
0218:     pub fn get_precharge(&self) -> Shr<Leafcell> {
0219:         self.cells.precharge.clone()
0220:     }
0221: }
0222: 
0223: impl Pdk {
0224:     pub fn load<P: AsRef<Path>>(pdk_path: P) -> YouRAMResult<Self> {
0225:         // load config
0226:         let pdk_path: &Path = pdk_path.as_ref();
0227:         let config = PdkConfig::load(pdk_path)?;
0228: 
0229:         // load file
0230:         let library = LibLibrary::load_file(config.stdcell_liberty_path()).map_err(PdkError::Liberty)?;
0231:         let stdcell_spice = Spice::load_from(config.stdcell_spice_path()).map_err(|e| YouRAMError::Message(e.to_string()))?;
0232:         let leafcell_spice = Spice::load_from(config.leafcell_spice_path()).map_err(|e| YouRAMError::Message(e.to_string()))?;
0233: 
0234:         // extract logicgates & dff
0235:         let cells = PdkCells::load(&library, &stdcell_spice, &leafcell_spice).context("load cells")?;
0236: 
0237:         // extract infomation 
0238:         let infomation = PdkInformation::load(&library, &cells)?;
0239: 
0240:         Ok(Self {
0241:             config,
0242:             cells,
0243:             infomation,
0244:         })
0245:     }
0246: }
0247: 
0248: #[cfg(test)]
0249: mod test {
0250:     use reda_sp::ToSpice;
0251:     use crate::{circuit::{DriveStrength, Primitive}, pdk::Process};
0252: 
0253:     use super::Pdk;
0254: 
0255:     #[test]
0256:     fn test_load_pdk() {
0257:         let pdk = Pdk::load("./platforms/nangate45").unwrap();
0258:         
0259:         let and2_x2 = pdk.get_and(2, DriveStrength::X2).unwrap();
0260:         println!("{}", and2_x2.read().netlist().to_spice());
0261: 
0262:         let bitcell = pdk.get_bitcell();
0263:         println!("{}", bitcell.read().netlist().to_spice());
0264: 
0265:         println!("{:?}", pdk.config.models.get(&Process::TypeType).unwrap());
0266: 
0267:         println!("{:?}", pdk.nmos_model_path(Process::TypeType).unwrap());
0268:         println!("{:?}", pdk.pdk_root_path());
0269:         println!("{}", pdk.name());
0270:         println!("{}", pdk.pvt());
0271: 
0272:         // println!("{:?}", pdk.get_dff(DriveStrength::X1).unwrap().read().setup_rising_timing);
0273: 
0274:         println!("{:?}", pdk.timing_input_net_transitions());
0275:         println!("{:?}", pdk.timing_output_net_capacitances());
0276:     }
0277: }

// File: YouRAM-master\src\pdk\types.rs

0001: use std::{fmt::Display, hash::Hash};
0002: 
0003: use reda_unit::{Capacitance, Temperature, Time, Voltage};
0004: use serde::{Deserialize, Serialize};
0005: 
0006: #[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
0007: pub enum Process {
0008:     #[serde(rename = "TT")]
0009:     TypeType,
0010: 
0011:     #[serde(rename = "FF")]
0012:     FastFast,
0013: 
0014:     #[serde(rename = "SS")]
0015:     SlowSlow,
0016: }
0017: 
0018: #[derive(Debug, Clone, Deserialize, Serialize)]
0019: pub struct Pvt {
0020:     pub process: Process,
0021:     pub voltage: Voltage,
0022:     pub temperature: Temperature,
0023: }
0024: 
0025: impl Pvt {
0026:     pub fn new<P, V, T>(process: P, voltage: V, temperature: T) -> Self 
0027:     where 
0028:         P: Into<Process>,
0029:         V: Into<Voltage>,
0030:         T: Into<Temperature>
0031:     {
0032:         Self {
0033:             process: process.into(),
0034:             voltage: voltage.into(),
0035:             temperature: temperature.into(),
0036:         }
0037:     }
0038: }
0039: 
0040: impl Display for Pvt {
0041:     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
0042:         write!(f, "P{}_V{}_T{}", self.process, self.voltage, self.temperature)
0043:     }
0044: }
0045: 
0046: #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
0047: pub struct SlewLoad {
0048:     pub slew: Time,
0049:     pub load: Capacitance,
0050: }
0051: 
0052: impl SlewLoad {
0053:     pub fn new<S, L>(slew: S, load: L) -> Self 
0054:     where 
0055:         S: Into<Time>,
0056:         L: Into<Capacitance>,
0057:     {
0058:         Self {
0059:             slew: slew.into(),
0060:             load: load.into(),
0061:         }
0062:     }
0063: }
0064: 
0065: impl Hash for SlewLoad {
0066:     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
0067:         state.write_u64(self.slew.value().to_f64().to_bits());
0068:         state.write_u64(self.load.value().to_f64().to_bits());
0069:     }
0070: }
0071: 
0072: impl Display for Process {
0073:     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
0074:         match self {
0075:             Self::TypeType => write!(f, "TT"),
0076:             Self::FastFast => write!(f, "FF"),
0077:             Self::SlowSlow => write!(f, "SS"),
0078:         }
0079:     }
0080: }
0081: 
0082: #[derive(Debug, Clone, Deserialize, Serialize)]
0083: pub struct Enviroment {
0084:     pvt: Pvt,
0085:     input_slew: Time,
0086:     output_load: Capacitance,
0087: }
0088: 
0089: impl Enviroment {
0090:     pub fn new(pvt: Pvt, input_slew: Time, output_load: Capacitance) -> Self {
0091:         Self {
0092:             pvt, input_slew, output_load
0093:         }
0094:     }
0095: 
0096:     pub fn process(&self) -> Process {
0097:         self.pvt.process
0098:     }
0099: 
0100:     pub fn voltage(&self) -> Voltage {
0101:         self.pvt.voltage
0102:     }
0103: 
0104:     pub fn temperature(&self) -> Temperature {
0105:         self.pvt.temperature
0106:     }
0107: 
0108:     pub fn input_slew(&self) -> Time {
0109:         self.input_slew
0110:     }
0111: 
0112:     pub fn output_load(&self) -> Capacitance {
0113:         self.output_load
0114:     }
0115: }

// File: YouRAM-master\src\simulate\error.rs

0001: use std::path::PathBuf;
0002: use super::MeasError;
0003: 
0004: #[derive(Debug, thiserror::Error)]
0005: pub enum SimulateError {
0006:     #[error("times len '{0}' != Voltages len '{1}'")]
0007:     TimesAndVoltageUnmatch(usize, usize),
0008: 
0009:     #[error("unsupport spice execute '{0}'")]
0010:     UnsupportExecute(String),
0011: 
0012:     #[error("execute command '{0}' failed for '{1}'")]
0013:     ExecuteError(String, String),
0014: 
0015:     #[error("invalid path '{0}'")]
0016:     InvalidPath(PathBuf),
0017: 
0018:     #[error("meas error: '{0}'")]
0019:     MeasError(#[from] MeasError),
0020: 
0021:     #[error("{msg} >> {err}")]
0022:     Context { msg: String, err: Box<SimulateError> }
0023: }   

// File: YouRAM-master\src\simulate\mod.rs

0001: mod meas;
0002: mod execute;
0003: mod write;
0004: mod error;
0005: pub use write::*;
0006: pub use meas::*;
0007: pub use error::*;
0008: pub use execute::*;
0009: 
0010: use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};
0011: use reda_unit::{t, v, Number, Time, Voltage};
0012: use itertools::Itertools;
0013: use crate::{circuit::{PortDirection, ShrCircuit}, pdk::{Enviroment, Pdk}, YouRAMResult};
0014: 
0015: pub struct CircuitSimulator {
0016:     pub writor: SpiceWritor,
0017:     pub circuit: ShrCircuit,
0018:     pub env: Enviroment,
0019:     pub pdk: Arc<Pdk>,
0020:     pub circuit_path: PathBuf,
0021: }
0022: 
0023: impl CircuitSimulator {
0024:     pub const VDD_PORT_NAME: &'static str = "VDD";
0025:     pub const GND_PORT_NAME: &'static str = "VSS";
0026:     pub const CLOSK_PORT_NAME: &'static str = "CLK";
0027: 
0028:     /// Create a circuit simulator, and write these auto:
0029:     /// - include file
0030:     /// - vdd/gnd source
0031:     /// - temperature
0032:     /// - instance of this circuit(all net has the same name with circuit's port)
0033:     /// 
0034:     /// Plase ensure `circuit` has been written in `circuit_path` 
0035:     /// 
0036:     /// after `create`, you may need to write:
0037:     /// - input stimulate 
0038:     /// - meas
0039:     /// - trans command
0040:     /// 
0041:     /// and last call `simulate` method to get result  
0042:     pub fn create<P1, P2, C>(
0043:         circuit: C, 
0044:         env: Enviroment,
0045:         pdk: Arc<Pdk>,
0046:         simulate_path: P1,
0047:         circuit_path: P2, 
0048:     ) -> YouRAMResult<Self> 
0049:     where 
0050:         P1: Into<PathBuf>,
0051:         P2: Into<PathBuf>,
0052:         C: Into<ShrCircuit>,
0053:     {        
0054:         let writor = SpiceWritor::open(simulate_path)?;
0055:         let mut simulator = Self { writor, circuit: circuit.into(), env, pdk, circuit_path: circuit_path.into() };
0056:         simulator.init()?;
0057:         Ok(simulator)
0058:     }
0059: 
0060:     fn init(&mut self) -> YouRAMResult<()> {
0061:         let nmos_model_path = self.pdk.nmos_model_path(self.env.process())?;
0062:         let pmos_model_path = self.pdk.pmos_model_path(self.env.process())?;
0063: 
0064:         // write includes 
0065:         self.writor.write_content("\n")?;
0066:         self.writor.write_include(nmos_model_path)?;
0067:         self.writor.write_include(pmos_model_path)?;
0068:         self.writor.write_include(&self.circuit_path)?;
0069:         self.writor.write_content("\n")?;
0070: 
0071:         // write enviroment
0072:         self.write_dc_stimulate(Self::VDD_PORT_NAME, self.env.voltage())?;
0073:         self.write_dc_stimulate(Self::GND_PORT_NAME, 0.0)?;
0074:         self.writor.write_temperature(self.env.temperature())?;
0075: 
0076:         // write circuit instance
0077:         let mut nets = vec![];
0078:         for port in self.circuit.ports().iter() {
0079:             match port.read().direction {
0080:                 PortDirection::Vdd => nets.push(Self::VDD_PORT_NAME.to_string()),
0081:                 PortDirection::Gnd => nets.push(Self::GND_PORT_NAME.to_string()),
0082:                 _ => nets.push(port.read().name.to_string()),
0083:                 
0084:             }
0085:         }
0086:         self.writor.write_instance(self.circuit.name(), self.circuit.name(), nets.into_iter())?;
0087:         for port in self.circuit.ports().iter() {
0088:             self.writor.write_capacitance(&port.read().name, &port.read().name, Self::GND_PORT_NAME, self.env.output_load())?;
0089:         }
0090:         self.writor.write_content("\n")?;
0091: 
0092:         Ok(())
0093:     }
0094: 
0095:     pub fn simulate(self, execute: &impl SpiceCommand, temp_folder: impl AsRef<Path>) -> YouRAMResult<HashMap<String, Number>> {
0096:         let mut executor = self.writor.close()?;
0097:         executor.simulate(execute, temp_folder.as_ref())
0098:     }
0099: }
0100: 
0101: impl CircuitSimulator {
0102:     pub fn logic1_voltage(&self) -> Voltage {
0103:         self.env.voltage()
0104:     } 
0105: 
0106:     pub fn logic0_voltage(&self) -> Voltage {
0107:         0.0.into()
0108:     }
0109: 
0110:     pub fn logic_voltage(&self, bit: bool) -> Voltage {
0111:         if bit { self.logic1_voltage() } else { self.logic0_voltage() }
0112:     }
0113: }
0114: 
0115: impl CircuitSimulator {
0116:     pub fn write_clock(&mut self, period: impl Into<Time>) -> YouRAMResult<()> {
0117:         let period = period.into();
0118:         self.writor.write_pulse_voltage(
0119:             Self::CLOSK_PORT_NAME, 
0120:             Self::CLOSK_PORT_NAME,
0121:             self.env.voltage(),
0122:             v!(0),
0123:             t!(0),
0124:             self.env.input_slew(),
0125:             self.env.input_slew(),
0126:             period / 2.0 - self.env.input_slew(),
0127:             period
0128:         )?;
0129:         Ok(())
0130:     }
0131: 
0132:     pub fn write_dc_stimulate(&mut self, port_name: impl AsRef<str>, voltage: impl Into<Voltage>) -> YouRAMResult<()> {
0133:         let port_name = port_name.as_ref();
0134:         self.writor.write_dc_voltage(port_name, port_name, voltage)
0135:     }
0136: 
0137:     pub fn write_period_stimulate(
0138:         &mut self,
0139:         port_name: impl AsRef<str>,
0140:         voltages: &[Voltage],
0141:         period: impl Into<Time>,
0142:         time_bias: impl Into<Time>, 
0143:     ) -> YouRAMResult<()> {
0144:         let port_name = port_name.as_ref();
0145:         let period = period.into();
0146:         let time_bias = time_bias.into();
0147: 
0148:         let times: Vec<_> = voltages.iter()
0149:             .enumerate()
0150:             .map(|(period_index, _)| {
0151:                 let time: Time = period_index as f64 * period + time_bias;
0152:                 time.max(t!(0))
0153:             })  
0154:             .collect();
0155:         self.writor.write_square_wave_voltage(port_name, port_name, &times, voltages, self.env.input_slew())
0156:     }
0157: 
0158:     pub fn write_square_wave_stimulate(
0159:         &mut self,
0160:         port_name: impl AsRef<str>,
0161:         time_voltages: impl Iterator<Item = (Time, Voltage)>,
0162:     ) -> YouRAMResult<()> {
0163:         let port_name = port_name.as_ref();
0164:         let (times, voltages): (Vec<_>, Vec<_>) = time_voltages.multiunzip();
0165:         self.writor.write_square_wave_voltage(port_name, port_name, &times, &voltages, self.env.input_slew())
0166:     }
0167: 
0168:     pub fn write_pwl_stimulate(
0169:         &mut self,
0170:         port_name: impl AsRef<str>,
0171:         time_voltages: impl Iterator<Item = (Time, Voltage)>,
0172:     ) -> YouRAMResult<()> {
0173:         let port_name = port_name.as_ref();
0174:         let (times, voltages): (Vec<_>, Vec<_>) = time_voltages.multiunzip();
0175:         self.writor.write_pwl_voltage(port_name, port_name, times.into_iter(), voltages.into_iter())
0176:     }
0177: 
0178:     #[inline]
0179:     pub fn write_logic1_stimulate(&mut self, port_name: impl AsRef<str>) -> YouRAMResult<()> {
0180:         self.write_dc_stimulate(port_name, self.env.voltage())
0181:     }
0182: 
0183:     #[inline]
0184:     pub fn write_logic0_stimulate(&mut self, port_name: impl AsRef<str>) -> YouRAMResult<()> {
0185:         self.write_dc_stimulate(port_name, 0.0)
0186:     }
0187: 
0188:     #[inline]
0189:     pub fn write_measurement(&mut self, meas: Box<dyn Meas>) -> YouRAMResult<()> {
0190:         self.writor.write_measurement(meas)
0191:     }
0192: 
0193:     #[inline]
0194:     pub fn write_trans(&mut self, step: impl Into<Time>, start: impl Into<Time>, end: impl Into<Time>) -> YouRAMResult<()> {
0195:         self.writor.write_trans(step, start, end)
0196:     }
0197: }

// File: YouRAM-master\src\simulate\execute\mod.rs

0001: mod ngspice;
0002: mod spectre;
0003: pub use ngspice::*;
0004: use reda_unit::Number;
0005: pub use spectre::*;
0006: use std::collections::HashMap;
0007: use std::path::{Path, PathBuf};
0008: use std::process::Command;
0009: use crate::{ErrorContext, YouRAMResult};
0010: use super::error::SimulateError;
0011: use super::Meas;
0012: 
0013: pub struct SpiceExector {
0014:     pub simulate_path: PathBuf,
0015:     pub measurements: Vec<Box<dyn Meas>>,
0016: }
0017: 
0018: impl SpiceExector {
0019:     pub fn simulate(&mut self, execute: &impl SpiceCommand, temp_folder: &Path) -> YouRAMResult<HashMap<String, Number>> {
0020:         let result_path = execute.execute(&self.simulate_path, temp_folder).context("Execute simualte")?;
0021:         self.get_meas_results(&result_path).context("Get meas result")
0022:     }   
0023: 
0024:     fn get_meas_results(&mut self, result_path: &Path) -> YouRAMResult<HashMap<String, Number>> {
0025:         let content = std::fs::read_to_string(result_path).context(format!("read result file '{:?}'", result_path))?;
0026: 
0027:         let mut results = HashMap::new();
0028:         for meas in self.measurements.iter() {
0029:             let value = meas.get_result(&content).map_err(SimulateError::MeasError)?;
0030:             results.insert(meas.name().to_string(), value);
0031:         }
0032: 
0033:         Ok(results)
0034:     }
0035: }
0036: 
0037: pub trait SpiceCommand {
0038:     /// Return the simulate command to execute 
0039:     fn simulate_command(&self, sim_filepath: &Path, temp_folder: &Path) -> YouRAMResult<String>;
0040: 
0041:     /// Return the meas filepath after simulate
0042:     fn meas_result_filepath(&self, sim_filepath: &Path, temp_folder: &Path) -> YouRAMResult<PathBuf> {
0043:         let temp_folder = temp_folder.to_path_buf();
0044: 
0045:         let filename = sim_filepath
0046:             .file_name()
0047:             .ok_or_else(|| SimulateError::InvalidPath(sim_filepath.to_path_buf()))?;
0048:         let mut filename = PathBuf::from(filename);
0049:         filename.set_extension("meas");
0050: 
0051:         Ok(temp_folder.join(filename)) 
0052:     }
0053: 
0054:     fn execute(&self, sim_filepath: &Path, temp_folder: &Path) -> YouRAMResult<PathBuf> {
0055:         let sim_filepath = sim_filepath.as_ref();
0056:         let temp_folder = temp_folder.as_ref();
0057: 
0058:         let command = self.simulate_command(sim_filepath, temp_folder)?;
0059:         let status = Command::new("sh")
0060:             .arg("-c")
0061:             .arg(&command)
0062:             .status()
0063:             .map_err(|e| SimulateError::ExecuteError(command.clone(), e.to_string()))?;
0064: 
0065:         match status.code() {
0066:             Some(0) => Ok(self.meas_result_filepath(sim_filepath, temp_folder)?),
0067:             Some(code) => Err(SimulateError::ExecuteError(command.clone(), format!("Command returns '{}'", code)))?,
0068:             None => Err(SimulateError::ExecuteError(command.clone(), "Command quit unnormal".into()))?,
0069:         }
0070:     }
0071: }
0072: 
0073: impl SpiceCommand for Box<dyn SpiceCommand> {
0074:     fn simulate_command(&self, sim_filepath: &Path, temp_folder: &Path) -> YouRAMResult<String> {
0075:         self.as_ref().simulate_command(sim_filepath, temp_folder)
0076:     }
0077: }

// File: YouRAM-master\src\simulate\execute\ngspice.rs

0001: use std::path::Path;
0002: use crate::YouRAMResult;
0003: use super::SpiceCommand;
0004: 
0005: #[derive(Clone)]
0006: pub struct NgSpice;
0007: 
0008: impl SpiceCommand for NgSpice {
0009:     fn simulate_command(&self, sim_filepath: &Path, temp_folder: &Path) -> YouRAMResult<String> {
0010:         let sim_filepath = sim_filepath.as_ref();
0011:         let temp_folder = temp_folder.as_ref();
0012: 
0013:         Ok(format!(
0014:             "ngspice -b -o {} {} > /dev/null 2>&1",
0015:             self.meas_result_filepath(sim_filepath, temp_folder)?.display(),
0016:             sim_filepath.display()
0017:         ))
0018:     }
0019: }

// File: YouRAM-master\src\simulate\execute\spectre.rs

0001: use crate::YouRAMResult;
0002: use super::SpiceCommand;
0003: use std::path::Path;
0004: 
0005: #[derive(Clone)]
0006: pub struct Spectre;
0007: 
0008: impl SpiceCommand for Spectre {
0009:     fn simulate_command(&self, sim_filepath: &Path, temp_folder: &Path) -> YouRAMResult<String> {
0010:         Ok(format!(
0011:             "spectre {} -outdir {} > /dev/null 2>&1",
0012:             sim_filepath.display(),
0013:             temp_folder.display(),
0014:         ))
0015:     }
0016: }

// File: YouRAM-master\src\simulate\meas\delay.rs

0001: use derive_builder::Builder;
0002: use reda_unit::{Time, Voltage};
0003: use super::Meas;
0004: 
0005: #[derive(Debug, Clone, Copy)]
0006: pub enum Edge {
0007:     Fall,
0008:     Rise,
0009: }
0010: 
0011: #[derive(Debug, Clone, Builder)]
0012: #[builder(pattern = "owned", setter(into))]
0013: pub struct DelayMeas {
0014:     pub name: String,
0015: 
0016:     pub trig_net_name: String,
0017:     pub trig_edge: Edge,
0018:     pub trig_voltage: Voltage,
0019:     pub trig_time_delay: Time,
0020: 
0021:     pub targ_net_name: String,
0022:     pub targ_edge: Edge,
0023:     pub targ_voltage: Voltage,
0024:     pub targ_time_delay: Time,
0025: }
0026: 
0027: impl Meas for DelayMeas {
0028:     fn name(&self) -> &str {
0029:         &self.name
0030:     }
0031: 
0032:     fn write_command(&self, out: &mut dyn std::io::Write) -> std::io::Result<()> {
0033:         let command = format!(
0034:             ".meas tran {} TRIG v({}) VAL={} {}=1 TD={} TARG v({}) VAL={} {}=1 TD={}\n",
0035:             
0036:             self.name,
0037: 
0038:             self.trig_net_name,
0039:             self.trig_voltage,
0040:             self.trig_edge,
0041:             self.trig_time_delay,
0042: 
0043:             self.targ_net_name,
0044:             self.targ_voltage,
0045:             self.targ_edge,
0046:             self.targ_time_delay,
0047:         );
0048: 
0049:         out.write_all(command.as_bytes())
0050:     }
0051: }
0052: 
0053: impl std::fmt::Display for Edge {
0054:     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
0055:         match self {
0056:             Self::Fall => write!(f, "FALL"),
0057:             Self::Rise => write!(f, "RISE"),
0058:         }
0059:     
0060:     }
0061: }

// File: YouRAM-master\src\simulate\meas\mod.rs

0001: mod voltageat;
0002: mod delay;
0003: 
0004: pub use voltageat::*;
0005: pub use delay::*;
0006: 
0007: use std::num::ParseFloatError;
0008: use regex::Regex;
0009: use reda_unit::Number;
0010: 
0011: #[derive(Debug, thiserror::Error)]
0012: pub enum MeasError {
0013:     #[error("meas '{0}' not found")]
0014:     NoMeasResultFound(String),
0015: 
0016:     #[error("parse value '{0}' failed for '{1}'")]
0017:     ParseValue(String, ParseFloatError),
0018: 
0019:     #[error("meas '{0}''s value not found")]
0020:     NoMeasValueFound(String),
0021: }
0022: 
0023: pub trait Meas {
0024:     fn name(&self) -> &str; 
0025:     fn write_command(&self, out: &mut dyn std::io::Write) -> std::io::Result<()>;
0026:     fn get_result(&self, context: &str) -> Result<Number, MeasError> {
0027:         let pattern = format!(r"{}\s*=\s*-?\d+\.?\d*[eE]?[-+]?\d+", regex::escape(&self.name()));
0028:         let re = Regex::new(&pattern).unwrap();
0029: 
0030:         let mat = match re.find(context) {
0031:             Some(m) => m,
0032:             None => return Err(MeasError::NoMeasResultFound(self.name().to_string())),
0033:         };
0034:         // "Vout = -1.2345e-3"
0035:         let name_value = mat.as_str();
0036: 
0037:         let eq_pos = name_value.find('=').unwrap();
0038:         let after_eq = &name_value[eq_pos + 1..].trim_start();
0039: 
0040:         let val = match after_eq.split_whitespace().next() {
0041:             Some(num_str) => match num_str.parse::<f64>() {
0042:                 Ok(v) => v,
0043:                 Err(e) => return Err(MeasError::ParseValue(num_str.into(), e))
0044:             },
0045:             None => return Err(MeasError::NoMeasValueFound(self.name().to_string())),
0046:         };
0047: 
0048:         Ok(Number::from_f64(val))
0049:     }
0050: }

// File: YouRAM-master\src\simulate\meas\voltageat.rs

0001: use reda_unit::Time;
0002: use super::Meas;
0003: 
0004: #[derive(Debug)]
0005: pub struct VoltageAtMeas {
0006:     pub name: String,
0007:     pub net_name: String,
0008:     pub meas_time: Time,
0009: }
0010: 
0011: impl VoltageAtMeas {
0012:     pub fn new<S1, S2, T>(name: S1, net_name: S2, meas_time: T) -> Self 
0013:     where 
0014:         S1: Into<String>,
0015:         S2: Into<String>,
0016:         T:  Into<Time>
0017:     {
0018:         Self { name: name.into(), net_name: net_name.into(), meas_time: meas_time.into() }
0019:     }
0020: }
0021: 
0022: impl Meas for VoltageAtMeas {
0023:     fn name(&self) -> &str {
0024:         &self.name
0025:     }
0026: 
0027:     fn write_command(&self, out: &mut dyn std::io::Write) -> std::io::Result<()> {
0028:         let command = format!(
0029:             ".meas tran {} FIND v({}) AT={}\n",
0030:             self.name,
0031:             self.net_name,
0032:             self.meas_time,
0033:         );
0034:         out.write_all(command.as_bytes())
0035:     }
0036: }

// File: YouRAM-master\src\simulate\write\mod.rs

0001: use std::fs::File;
0002: use std::io::Write;
0003: use std::path::{Path, PathBuf};
0004: use reda_unit::{Capacitance, Temperature, Time, Voltage};
0005: use crate::YouRAMResult;
0006: use crate::simulate::{Meas, SimulateError};
0007: 
0008: use super::SpiceExector;
0009: 
0010: pub struct SpiceWritor {
0011:     simulate_path: PathBuf,
0012:     file: File,
0013:     measurements: Vec<Box<dyn Meas>>, 
0014: }
0015: 
0016: impl SpiceWritor {
0017:     pub fn open<P: Into<PathBuf>>(simulate_path: P) -> YouRAMResult<Self> {
0018:         let simulate_path = simulate_path.into();
0019:         let file = File::create(&simulate_path)?;
0020:         Ok(Self {
0021:             simulate_path: simulate_path.to_path_buf(),
0022:             file,
0023:             measurements: vec![],
0024:         })
0025:     }
0026: 
0027:     pub fn close(mut self) -> YouRAMResult<SpiceExector> {
0028:         self.file.flush()?;
0029:         Ok(SpiceExector {
0030:             simulate_path: self.simulate_path,
0031:             measurements: self.measurements
0032:         })
0033:     }
0034: }
0035: 
0036: #[allow(dead_code)]
0037: impl SpiceWritor {
0038:     pub fn simulate_path(&self) -> &Path {
0039:         &self.simulate_path
0040:     }
0041: 
0042:     pub fn write_content(&mut self, content: impl AsRef<str>) -> YouRAMResult<()> {
0043:         write!(self.file, "{}", content.as_ref())?;
0044:         Ok(())
0045:     }
0046: 
0047:     pub fn write_include<P: AsRef<Path>>(&mut self, path: P) -> YouRAMResult<()> {
0048:         writeln!(self.file, ".include {}", path.as_ref().display())?;
0049:         Ok(())
0050:     }
0051: 
0052:     pub fn write_end(&mut self) -> YouRAMResult<()> {
0053:         writeln!(self.file, ".end")?;
0054:         Ok(())
0055:     }
0056: 
0057:     pub fn write_comment(&mut self, comment: impl AsRef<str>) -> YouRAMResult<()> {
0058:         writeln!(self.file, "* {}", comment.as_ref())?;
0059:         Ok(())
0060:     }
0061: 
0062:     pub fn write_temperature(&mut self, temp: impl Into<Temperature>) -> YouRAMResult<()> {
0063:         let temp: Temperature = temp.into();
0064:         writeln!(self.file, ".TEMP {}", temp.value())?;
0065:         Ok(())
0066:     }
0067: 
0068:     pub fn write_instance(
0069:         &mut self,
0070:         module_name: impl AsRef<str>,
0071:         instance_name: impl AsRef<str>,
0072:         nets: impl Iterator<Item = impl AsRef<str>>,
0073:     ) -> YouRAMResult<()> {
0074:         write!(self.file, "X{}", instance_name.as_ref())?;
0075:         for net in nets {
0076:             write!(self.file, " {}", net.as_ref())?;
0077:         }
0078:         writeln!(self.file, " {}", module_name.as_ref())?;
0079:         Ok(())
0080:     }
0081: 
0082:     pub fn write_pwl_voltage(
0083:         &mut self,
0084:         voltage_name: impl AsRef<str>,
0085:         net_name: impl AsRef<str>,
0086:         times: impl ExactSizeIterator<Item = Time>,
0087:         voltages: impl ExactSizeIterator<Item = Voltage>,
0088:     ) -> YouRAMResult<()> {
0089:         if times.len() != voltages.len() {
0090:             return Err(SimulateError::TimesAndVoltageUnmatch(times.len(), voltages.len()))?;
0091:         }
0092: 
0093:         write!(self.file, "V{} {} 0 PWL (", voltage_name.as_ref(), net_name.as_ref())?;
0094:         for (t, v) in times.zip(voltages) {
0095:             write!(self.file, "{} {} ", t, v)?;
0096:         }
0097:         writeln!(self.file, ")")?;
0098: 
0099:         Ok(())
0100:     }
0101: 
0102:     pub fn write_square_wave_voltage(
0103:         &mut self,
0104:         voltage_name: impl AsRef<str>,
0105:         net_name: impl AsRef<str>,
0106:         times: &[Time],
0107:         voltages: &[Voltage],
0108:         slew: impl Into<Time>,
0109:     ) -> YouRAMResult<()> {
0110:         let slew = slew.into();
0111:         if times.len() != voltages.len() {
0112:             return Err(SimulateError::TimesAndVoltageUnmatch(times.len(), voltages.len()))?;
0113:         }
0114:         
0115:         write!(self.file, "V{} {} 0 PWL (", voltage_name.as_ref(), net_name.as_ref())?;
0116:         if times.is_empty() {
0117:             writeln!(self.file, ")")?;
0118:             return Ok(())
0119:         }
0120: 
0121:         write!(self.file, "{} {} ", times[0], voltages[0])?;
0122:         
0123:         for i in 1..times.len() {
0124:             write!(self.file, "{} {} ", times[i] - slew, voltages[i - 1])?;
0125:             write!(self.file, "{} {} ", times[i] + slew, voltages[i])?
0126:         }
0127:         
0128:         writeln!(self.file, ")")?;
0129:         Ok(())
0130:     }
0131: 
0132:     pub fn write_pulse_voltage(
0133:         &mut self,
0134:         voltage_name: impl AsRef<str>,
0135:         net_name: impl AsRef<str>,
0136:         init_voltage: impl Into<Voltage>,
0137:         pulse_voltage: impl Into<Voltage>,
0138:         delay: impl Into<Time>,
0139:         rise: impl Into<Time>,
0140:         fall: impl Into<Time>,
0141:         width: impl Into<Time>,
0142:         period: impl Into<Time>,
0143:     ) -> YouRAMResult<()> {
0144:         writeln!(
0145:             self.file,
0146:             "V{} {} 0 PULSE({} {} {} {} {} {} {})",
0147:             voltage_name.as_ref(),
0148:             net_name.as_ref(),
0149:             init_voltage.into(), pulse_voltage.into(),
0150:             delay.into(), rise.into(), fall.into(),
0151:             width.into(), period.into()
0152:         )?;
0153: 
0154:         Ok(())
0155:     }
0156: 
0157:     pub fn write_dc_voltage(
0158:         &mut self,
0159:         voltage_name: impl AsRef<str>,
0160:         net_name: impl AsRef<str>,
0161:         voltage: impl Into<Voltage>,
0162:     ) -> YouRAMResult<()> {
0163:         writeln!(self.file, "V{} {} 0 {}", voltage_name.as_ref(), net_name.as_ref(), voltage.into())?;
0164:         Ok(())
0165:     }
0166: 
0167:     pub fn write_capacitance(
0168:         &mut self,
0169:         name: impl AsRef<str>,
0170:         n1: impl AsRef<str>,
0171:         n2: impl AsRef<str>,
0172:         value: impl Into<Capacitance>,
0173:     ) -> YouRAMResult<()> {
0174:         writeln!(self.file, "C{} {} {} {}", name.as_ref(), n1.as_ref(), n2.as_ref(), value.into())?;
0175:         Ok(())
0176:     }
0177: 
0178:     pub fn write_trans(&mut self, step: impl Into<Time>, start: impl Into<Time>, end: impl Into<Time>) -> YouRAMResult<()> {
0179:         writeln!(self.file, ".TRAN {} {} {}", step.into(), end.into(), start.into())?;
0180:         Ok(())
0181:     }
0182: 
0183:     pub fn write_measurement(&mut self, meas: Box<dyn Meas>) -> YouRAMResult<()> {
0184:         meas.write_command(&mut self.file)?;
0185:         self.measurements.push(meas);
0186:         Ok(())
0187:     }
0188: }
0189: 
0190: #[cfg(test)]
0191: mod tests {
0192:     use super::*;
0193:     use std::io::Read;
0194:     use reda_unit::{num, t, v};
0195:     use tempfile::NamedTempFile;
0196:     use std::fs::OpenOptions;
0197: 
0198:     fn read_file_to_string(path: &PathBuf) -> String {
0199:         let mut f = std::fs::File::open(path).unwrap();
0200:         let mut content = String::new();
0201:         f.read_to_string(&mut content).unwrap();
0202:         content
0203:     }
0204: 
0205:     #[test]
0206:     fn test_basic_commands_written() {
0207:         let tmp = NamedTempFile::new().unwrap();
0208:         let path = tmp.path().to_path_buf();
0209: 
0210:         let file = OpenOptions::new().read(true).write(true).open(&path).unwrap();
0211:         let mut sim = SpiceWritor {
0212:             simulate_path: path.clone(),
0213:             file,
0214:             measurements: vec![],
0215:         };
0216: 
0217:         sim.write_include("model.sp").unwrap();
0218:         sim.write_comment("test comment").unwrap();
0219:         sim.write_temperature(num!(27)).unwrap();
0220:         sim.write_end().unwrap();
0221:         sim.file.flush().unwrap();
0222: 
0223:         let content = read_file_to_string(&path);
0224:         assert!(content.contains(".include model.sp"));
0225:         assert!(content.contains("* test comment"));
0226:         assert!(content.contains(".TEMP 27"));
0227:         assert!(content.contains(".end"));
0228:     }
0229: 
0230:     #[test]
0231:     fn test_voltage_sources() {
0232:         let tmp = NamedTempFile::new().unwrap();
0233:         let path = tmp.path().to_path_buf();
0234:         let file = OpenOptions::new().read(true).write(true).open(&path).unwrap();
0235: 
0236:         let mut sim = SpiceWritor {
0237:             simulate_path: path.clone(),
0238:             file,
0239:             measurements: vec![],
0240:         };
0241: 
0242:         sim.write_dc_voltage("VDD", "vdd", num!(1.2)).unwrap();
0243:         sim.write_pulse_voltage(
0244:             "CLK", "clk", num!(0.0), num!(1.8),
0245:             num!(1 n), num!(0.1 n), num!(0.1 n),
0246:             num!(4.9 n), num!(10 n)
0247:         ).unwrap();
0248: 
0249:         sim.write_pwl_voltage(
0250:             "IN", "in", 
0251:             [t!(0), t!(1 n)].into_iter(), 
0252:             [v!(0), v!(1.8)].into_iter(), 
0253:         ).unwrap();
0254: 
0255:         sim.file.flush().unwrap();
0256:         let content = read_file_to_string(&path);
0257:         println!("{}", content);
0258:         assert!(content.contains("VDD vdd 0 1.2"));
0259:         assert!(content.contains("PULSE("));
0260:         assert!(content.contains("PWL"));
0261:     }
0262: 
0263:     #[test]
0264:     fn test_instance_and_cap() {
0265:         let tmp = NamedTempFile::new().unwrap();
0266:         let path = tmp.path().to_path_buf();
0267:         let file = OpenOptions::new().read(true).write(true).open(&path).unwrap();
0268: 
0269:         let mut sim = SpiceWritor {
0270:             simulate_path: path.clone(),
0271:             file,
0272:             measurements: vec![],
0273:         };
0274: 
0275:         sim.write_instance("inv", "inv0", ["in", "out", "vdd", "gnd"].iter()).unwrap();
0276:         sim.write_capacitance("load", "out", "gnd", num!(0.01)).unwrap();
0277:         sim.file.flush().unwrap();
0278: 
0279:         let content = read_file_to_string(&path);
0280:         assert!(content.contains("Xinv0 in out vdd gnd inv"));
0281:         assert!(content.contains("Cload out gnd 0.01"));
0282:     }
0283: }

// File: YouRAM-master\tests\andarray.rs

0001: use std::sync::Arc;
0002: use rand::Rng;
0003: use reda_unit::t;
0004: use tracing::{info, Level};
0005: use youram::{
0006:     circuit::{AndArray, AndArrayArg, CircuitFactory}, export, pdk::{Enviroment, Pdk}, simulate::{CircuitSimulator, NgSpice, VoltageAtMeas}, ErrorContext
0007: };
0008: use approx::assert_abs_diff_eq;
0009: 
0010: const PDK: &str = "./platforms/nangate45";
0011: const TEMP: &str = "./temp";
0012: const AND_SIZE: usize = 8;
0013: 
0014: fn main_result() -> Result<(), Box<dyn std::error::Error>> {
0015:     tracing_subscriber::fmt()
0016:         .with_max_level(Level::DEBUG)
0017:         .with_target(false)
0018:         .with_file(false)
0019:         .with_line_number(false)
0020:         .init();
0021: 
0022:     let pdk = Arc::new(Pdk::load(PDK).context("load pdk")?);
0023:     let mut factory = CircuitFactory::new(pdk.clone());
0024:     let andarray = factory.module(AndArrayArg::new(AND_SIZE))?;
0025:     let pvt = pdk.pvt();
0026:     let env = Enviroment::new(pvt.clone(), t!(0.5 n), 0.0.into());
0027: 
0028:     export::write_spice(andarray.clone(), format!("{TEMP}/andarray.sp"))?;
0029: 
0030:     // test en = 1
0031:     {
0032:         let mut simulator = CircuitSimulator::create(
0033:             andarray.clone(), 
0034:             env.clone(), 
0035:             pdk.clone(), 
0036:             format!("{TEMP}/simulate.sp"), 
0037:             format!("{TEMP}/andarray.sp"), 
0038:         )?;
0039:     
0040:         simulator.write_logic1_stimulate(AndArray::enbale_pn())?;
0041:         let inputs: Vec<bool> = rand::rng().random_iter().take(AND_SIZE).collect();
0042:         for (i, input) in inputs.iter().enumerate() {
0043:             if *input {
0044:                 simulator.write_logic1_stimulate(AndArray::input_pn(i))?;
0045:             } else {
0046:                 simulator.write_logic0_stimulate(AndArray::input_pn(i))?;
0047:             }
0048:         }
0049: 
0050:         for i in 0..inputs.len() {
0051:             let meas = VoltageAtMeas::new(format!("output{i}"), AndArray::output_pn(i).to_string(), t!(10. n));
0052:             simulator.write_measurement(Box::new(meas))?;
0053:         }
0054: 
0055:         simulator.write_trans(t!(0.5 n), t!(0.0), t!(15. n))?;
0056:         let result = simulator.simulate(&NgSpice, TEMP)?;
0057: 
0058:         for (index, expect_value) in inputs.into_iter().enumerate() {
0059:             let expect_value = if expect_value { pvt.voltage.to_f64() } else { 0.0 }; 
0060:             let name = format!("output{index}");
0061:             let got_value = result.get(&name).unwrap().to_f64();
0062:             info!("{name}: got {got_value}, expect {expect_value}");
0063:             assert_abs_diff_eq!(expect_value, got_value, epsilon = 1e-2);
0064:         }
0065:     }
0066: 
0067:     // test en = 0
0068:     {
0069:         let mut simulator = CircuitSimulator::create(
0070:             andarray.clone(), 
0071:             env.clone(), 
0072:             pdk.clone(), 
0073:             format!("{TEMP}/simulate.sp"), 
0074:             format!("{TEMP}/andarray.sp"), 
0075:         )?;
0076:     
0077:         simulator.write_logic0_stimulate(AndArray::enbale_pn())?;
0078:         let inputs: Vec<bool> = rand::rng().random_iter().take(AND_SIZE).collect();
0079:         for (i, input) in inputs.iter().enumerate() {
0080:             if *input {
0081:                 simulator.write_logic1_stimulate(AndArray::input_pn(i))?;
0082:             } else {
0083:                 simulator.write_logic0_stimulate(AndArray::input_pn(i))?;
0084:             }
0085:         }
0086: 
0087:         for i in 0..inputs.len() {
0088:             let meas = VoltageAtMeas::new(format!("output{i}"), AndArray::output_pn(i).to_string(), t!(10. n));
0089:             simulator.write_measurement(Box::new(meas))?;
0090:         }
0091: 
0092:         simulator.write_trans(t!(0.5 n), t!(0.0), t!(15. n))?;
0093:         let result = simulator.simulate(&NgSpice, TEMP)?;
0094: 
0095:         for (name, got_value) in result {
0096:             info!("{name}: got {got_value}");
0097:             assert_abs_diff_eq!(0.0, got_value.to_f64(), epsilon = 1e-2);
0098:         }
0099:     }
0100: 
0101:     Ok(())
0102: }
0103: 
0104: #[test]
0105: fn main() {
0106:     if let Err(e) = main_result() {
0107:         eprint!("Err: {}\n", e);
0108:         panic!("");
0109:     }
0110: }

// File: YouRAM-master\tests\decoder.rs

0001: use std::sync::Arc;
0002: use reda_unit::t;
0003: use tracing::{info, Level};
0004: use youram::{
0005:     circuit::{CircuitFactory, Decoder, DecoderArg}, export, pdk::{Enviroment, Pdk}, simulate::{CircuitSimulator, NgSpice, VoltageAtMeas}, ErrorContext
0006: };
0007: use approx::assert_abs_diff_eq;
0008: 
0009: const PDK: &str = "./platforms/nangate45";
0010: const TEMP: &str = "./temp";
0011: const INPUT_SIZE: usize = 4;
0012: const OUTPUT_SIZE: usize = 2usize.pow(INPUT_SIZE as u32);
0013: 
0014: fn main_result() -> Result<(), Box<dyn std::error::Error>> {
0015:     tracing_subscriber::fmt()
0016:         .with_max_level(Level::INFO)
0017:         .with_target(false)
0018:         .with_file(false)
0019:         .with_line_number(false)
0020:         .init();
0021: 
0022:     let pdk = Arc::new(Pdk::load(PDK).context("load pdk")?);
0023:     let mut factory = CircuitFactory::new(pdk.clone());
0024:     let decoder = factory.module(DecoderArg::new(INPUT_SIZE))?;
0025:     let pvt = pdk.pvt();
0026:     let env = Enviroment::new(pvt.clone(), t!(0.5 n), 0.0.into());
0027: 
0028:     export::write_spice(decoder.clone(), format!("{TEMP}/andarray.sp"))?;
0029: 
0030:     for t in 0..OUTPUT_SIZE {
0031:         info!("test input {t}");
0032:         let mut simulator = CircuitSimulator::create(
0033:             decoder.clone(), 
0034:             env.clone(), 
0035:             pdk.clone(), 
0036:             format!("{TEMP}/simulate.sp"), 
0037:             format!("{TEMP}/decoder.sp"), 
0038:         )?;
0039:         
0040:         for input_index in 0..INPUT_SIZE {
0041:             let is_logic1 = (t & 0x1 << input_index) != 0;
0042:             if is_logic1 {
0043:                 simulator.write_logic1_stimulate(Decoder::address_pn(input_index))?;
0044:             } else {
0045:                 simulator.write_logic0_stimulate(Decoder::address_pn(input_index))?;
0046:             }
0047:         }
0048:      
0049:         for i in 0..OUTPUT_SIZE {
0050:             let meas = VoltageAtMeas::new(format!("output{i}"), Decoder::output_pn(i).to_string(), t!(10. n));
0051:             simulator.write_measurement(Box::new(meas))?;
0052:         }
0053:         simulator.write_trans(t!(0.5 n), t!(0.0), t!(15. n))?;
0054: 
0055:         let result = simulator.simulate(&NgSpice, TEMP)?;
0056:         
0057:         for (name, value) in result {
0058:             info!("{name}: {value}");
0059:             if name == format!("output{t}") {
0060:                 assert_abs_diff_eq!(value.to_f64(), pvt.voltage.to_f64(), epsilon = 1e-2);
0061:             } else {
0062:                 assert_abs_diff_eq!(value.to_f64(), 0.0, epsilon = 1e-2);
0063:             }
0064:         }   
0065:     }
0066: 
0067:     Ok(())
0068: }
0069: 
0070: #[test]
0071: fn main() {
0072:     if let Err(e) = main_result() {
0073:         eprint!("Err: {}\n", e);
0074:         panic!("");
0075:     }
0076: }

// File: YouRAM-master\tests\sram.rs

0001: use std::sync::Arc;
0002: use reda_unit::t;
0003: use tracing::Level;
0004: use youram::{
0005:     charz::{FunctionCharz, RandomPolicy}, 
0006:     circuit::{CircuitFactory, SramArg}, 
0007:     pdk::{Enviroment, Pdk}, 
0008:     simulate::NgSpice, ErrorContext
0009: };
0010: 
0011: const PDK: &str = "./platforms/nangate45";
0012: const TEMP: &str = "./temp";
0013: const ADDRESS_WIDTH: usize = 2;
0014: const WORD_WIDTH: usize = 4;
0015: 
0016: fn main_result() -> Result<(), Box<dyn std::error::Error>> {
0017:     tracing_subscriber::fmt()
0018:         .with_max_level(Level::DEBUG)
0019:         .with_target(false)
0020:         .with_file(false)
0021:         .with_line_number(false)
0022:         .init();
0023: 
0024:     let pdk = Arc::new(Pdk::load(PDK).context("load pdk")?);
0025:     let mut factory = CircuitFactory::new(pdk.clone());
0026:     let sram = factory.module(SramArg::new(ADDRESS_WIDTH, WORD_WIDTH))?;
0027:     let pvt = pdk.pvt();
0028:     let env = Enviroment::new(pvt.clone(), t!(0.5 n), 0.0.into());
0029: 
0030:     let pass = FunctionCharz::config()
0031:         .sram(sram.clone())
0032:         .period(t!(10. n))
0033:         .env(env)
0034:         .pdk(pdk)
0035:         .policy(RandomPolicy)
0036:         .command(NgSpice)
0037:         .temp_folder(TEMP)
0038:         .test()?;
0039: 
0040:     assert!(pass);
0041: 
0042:     Ok(())
0043: }
0044: 
0045: #[test]
0046: fn main() {
0047:     if let Err(e) = main_result() {
0048:         eprint!("Err: {}\n", e);
0049:         panic!("");
0050:     }
0051: }

// File: YouRAM-master\tests\sram_liberty.rs

0001: use std::sync::Arc;
0002: use reda_unit::t;
0003: use tracing::{error, Level};
0004: use youram::{
0005:     circuit::{CircuitFactory, SramArg}, export, pdk::Pdk, simulate::NgSpice, ErrorContext
0006: };
0007: 
0008: const PDK: &str = "./platforms/nangate45";
0009: const TEMP: &str = "./temp";
0010: const ADDRESS_WIDTH: usize = 4;
0011: const WORD_WIDTH: usize = 4;
0012: 
0013: fn main_result() -> Result<(), Box<dyn std::error::Error>> {
0014:     tracing_subscriber::fmt()
0015:         .with_max_level(Level::DEBUG)
0016:         .with_target(false)
0017:         .with_file(false)
0018:         .with_line_number(false)
0019:         .init();
0020: 
0021:     let pdk = Arc::new(Pdk::load(PDK).context("load pdk")?);
0022:     let mut factory = CircuitFactory::new(pdk.clone());
0023:     let sram = factory.module(SramArg::new(ADDRESS_WIDTH, WORD_WIDTH))?;
0024: 
0025:     export::write_liberty(
0026:         sram.clone(), 
0027:         format!("{}/sram.lib", TEMP),
0028:         t!(10. n), 
0029:         pdk, 
0030:         Box::new(NgSpice), 
0031:         TEMP
0032:     )?;
0033: 
0034:     Ok(())
0035: }
0036: 
0037: #[test]
0038: fn main() {
0039:     if let Err(e) = main_result() {
0040:         error!("Err: {}\n", e);
0041:         panic!("");
0042:     }
0043: }

// File: YouRAM-master\tests\sram_timing.rs

0001: use std::sync::Arc;
0002: use reda_unit::t;
0003: use tracing::{error, info, Level};
0004: use youram::{
0005:     charz::TimingCharz, circuit::{CircuitFactory, SramArg}, pdk::Pdk, simulate::NgSpice, ErrorContext
0006: };
0007: 
0008: const PDK: &str = "./platforms/nangate45";
0009: const TEMP: &str = "./temp";
0010: const ADDRESS_WIDTH: usize = 4;
0011: const WORD_WIDTH: usize = 4;
0012: 
0013: fn main_result() -> Result<(), Box<dyn std::error::Error>> {
0014:     tracing_subscriber::fmt()
0015:         .with_max_level(Level::DEBUG)
0016:         .with_target(false)
0017:         .with_file(false)
0018:         .with_line_number(false)
0019:         .init();
0020: 
0021:     let pdk = Arc::new(Pdk::load(PDK).context("load pdk")?);
0022:     let mut factory = CircuitFactory::new(pdk.clone());
0023:     let sram = factory.module(SramArg::new(ADDRESS_WIDTH, WORD_WIDTH))?;
0024:     let pvt = pdk.pvt();
0025: 
0026:     let result = TimingCharz::config()
0027:         .sram(sram.clone())
0028:         .period(t!(10. n))
0029:         .pvt(pvt.clone())
0030:         .input_net_transitions(&[t!(0.5 n)])
0031:         .output_net_capacitances(&[0.0.into()])
0032:         .pdk(pdk)
0033:         .command(NgSpice)
0034:         .temp_folder(TEMP)
0035:         .analyze()?;
0036: 
0037:     info!("Timing charz result: {:#?}", result);
0038: 
0039:     Ok(())
0040: }
0041: 
0042: #[test]
0043: fn main() {
0044:     if let Err(e) = main_result() {
0045:         error!("Err: {}\n", e);
0046:         panic!("");
0047:     }
0048: }
