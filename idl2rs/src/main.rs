use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use std::borrow::Cow;
use std::io::{self, Read, Write};

#[derive(Parser)]
#[grammar = "idl.pest"]
struct IDLParser;

#[derive(Debug)]
enum Modifier {
    Pointer,
    Const,
}

#[derive(Debug, Default)]
struct Type<'a> {
    base_type: Cow<'a, str>,
    modifiers: Vec<Modifier>,
}

impl<'a> Type<'a> {
    fn from_pest(pair: Pair<'a, Rule>) -> Self {
        assert_eq!(pair.as_rule(), Rule::_type);

        let mut result = Self::default();

        for p in pair.into_inner() {
            match p.as_rule() {
                Rule::identifier => {
                    result.base_type = if p.as_str().eq_ignore_ascii_case("int") {
                        "i32".into()
                    } else if p.as_str().eq_ignore_ascii_case("double") {
                        "f64".into()
                    } else if p.as_str().starts_with("I") {
                        result.modifiers.push(Modifier::Pointer);
                        format!("{}VTable", p.as_str()).into()
                    } else {
                        p.as_str().into()
                    }
                }
                Rule::pointer => result.modifiers.push(Modifier::Pointer),
                Rule::_const => result.modifiers.push(Modifier::Const),
                _ => {}
            }
        }
        result.modifiers.reverse();
        result
    }

    pub fn render(&self, w: &mut impl Write) -> io::Result<()> {
        for m in &self.modifiers {
            if matches!(m, Modifier::Pointer) {
                write!(w, "*mut ")?;
            }
        }

        write!(w, "{}", self.base_type)
    }
}

#[derive(Debug, Default)]
struct Parameter<'a> {
    attributes: Vec<&'a str>,
    r#type: Type<'a>,
    name: &'a str,
}

impl<'a> Parameter<'a> {
    fn from_pest(pair: Pair<'a, Rule>) -> Self {
        assert_eq!(pair.as_rule(), Rule::parameter);

        let mut result = Self::default();

        for p in pair.into_inner() {
            match p.as_rule() {
                Rule::parameter_attribute => result.attributes.push(p.as_str()),
                Rule::_type => result.r#type = Type::from_pest(p),
                Rule::identifier => result.name = p.as_str(),
                _ => {}
            }
        }
        result
    }

    pub fn render(&self, w: &mut impl Write) -> io::Result<()> {
        if !self.attributes.is_empty() {
            write!(w, "/* {} */ ", self.attributes.join(", "))?;
        };
        write!(w, "{}: ", self.name)?;
        self.r#type.render(w)
    }
}

#[derive(Debug, Default)]
struct Method<'a> {
    doc_comment: Option<&'a str>,
    return_type: Type<'a>,
    name: &'a str,
    parameters: Vec<Parameter<'a>>,
}

impl<'a> Method<'a> {
    fn from_pest(pair: Pair<'a, Rule>) -> Self {
        assert_eq!(pair.as_rule(), Rule::method);

        let mut result = Self::default();

        for p in pair.into_inner() {
            match p.as_rule() {
                Rule::doc_comment => result.doc_comment = Some(p.as_str().trim_end_matches(" \t")),
                Rule::_type => result.return_type = Type::from_pest(p),
                Rule::method_name => result.name = p.as_str(),
                Rule::parameter => result.parameters.push(Parameter::from_pest(p)),
                _ => {}
            }
        }
        result
    }

    pub fn render(&self, w: &mut impl Write) -> io::Result<()> {
        write!(w, "{}", self.doc_comment.unwrap_or(""))?;
        write!(w, "    unsafe fn {}(&self", camel_to_snake(self.name))?;
        for p in &self.parameters {
            write!(w, ", ")?;
            p.render(w)?;
        }
        write!(w, ") -> ")?;
        self.return_type.render(w)?;
        writeln!(w, ";")
    }
}

#[derive(Debug, Default)]
struct TypedefEnum<'a> {
    doc_comment: Option<&'a str>,
    name: &'a str,
    variants: Vec<Variant<'a>>,
}

#[derive(Debug, Default)]
struct Variant<'a> {
    doc_comment: Option<&'a str>,
    name: &'a str,
}

impl<'a> TypedefEnum<'a> {
    fn from_pest(pair: Pair<'a, Rule>) -> Self {
        assert_eq!(pair.as_rule(), Rule::typedef_enum);

        let mut result = Self::default();

        for p in pair.into_inner() {
            match p.as_rule() {
                Rule::doc_comment => result.doc_comment = Some(p.as_str().trim_end_matches(" \t")),
                Rule::identifier => result.name = p.as_str(),
                Rule::variant => {
                    let variant = {
                        let mut result = Variant::default();
                        for p in p.into_inner() {
                            match p.as_rule() {
                                Rule::doc_comment => {
                                    result.doc_comment = Some(p.as_str().trim_end_matches(" \t"))
                                }
                                Rule::identifier => result.name = p.as_str(),
                                _ => {}
                            }
                        }
                        result
                    };
                    result.variants.push(variant);
                }
                _ => {}
            }
        }
        result
    }

    pub fn render(&self, w: &mut impl Write) -> io::Result<()> {
        write!(w, "{}", self.doc_comment.unwrap_or(""))?;
        writeln!(w, "#[repr(u32)]")?;
        writeln!(w, "pub enum {} {{", self.name)?;
        for variant in &self.variants {
            write!(w, "{}", variant.doc_comment.unwrap_or(""))?;
            writeln!(w, "    {},", variant.name)?;
        }
        writeln!(w, "}}")
    }
}

#[derive(Debug, Default)]
struct Field<'a> {
    doc_comment: Option<&'a str>,
    name: &'a str,
    r#type: Type<'a>,
}

impl<'a> Field<'a> {
    fn from_pest(pair: Pair<'a, Rule>) -> Self {
        assert_eq!(pair.as_rule(), Rule::field);

        let mut result = Self::default();

        for p in pair.into_inner() {
            match p.as_rule() {
                Rule::doc_comment => result.doc_comment = Some(p.as_str().trim_end_matches(" \t")),
                Rule::_type => result.r#type = Type::from_pest(p),
                Rule::identifier => result.name = p.as_str(),
                _ => {}
            }
        }
        result
    }
}

#[derive(Debug, Default)]
struct TypedefStruct<'a> {
    doc_comment: Option<&'a str>,
    name: &'a str,
    fields: Vec<Field<'a>>,
}

impl<'a> TypedefStruct<'a> {
    fn from_pest(pair: Pair<'a, Rule>) -> Self {
        assert_eq!(pair.as_rule(), Rule::typedef_struct);

        let mut result = Self::default();

        for p in pair.into_inner() {
            match p.as_rule() {
                Rule::doc_comment => result.doc_comment = Some(p.as_str().trim_end_matches(" \t")),
                Rule::identifier => result.name = p.as_str(),
                Rule::field => result.fields.push(Field::from_pest(p)),
                _ => {}
            }
        }
        result
    }

    fn render(&self, w: &mut impl Write) -> io::Result<()> {
        write!(w, "{}", self.doc_comment.unwrap_or(""))?;
        writeln!(w, "#[repr(C)]")?;
        writeln!(w, "pub struct {} {{", self.name)?;
        for field in &self.fields {
            write!(w, "{}", field.doc_comment.unwrap_or(""))?;
            write!(w, "    {}: ", field.name)?;
            field.r#type.render(w)?;
            writeln!(w, ",")?;
        }
        writeln!(w, "}}")
    }
}

#[derive(Debug, Default)]
struct Interface<'a> {
    doc_comment: Option<&'a str>,
    name: &'a str,
    parent: &'a str,
    uuid: Option<&'a str>,
    attributes: Vec<&'a str>,
    enums: Vec<TypedefEnum<'a>>,
    structs: Vec<TypedefStruct<'a>>,
    methods: Vec<Method<'a>>,
}

impl<'a> Interface<'a> {
    fn from_pest(pair: Pair<'a, Rule>) -> Self {
        assert_eq!(pair.as_rule(), Rule::interface);

        let mut result = Self::default();

        for p in pair.into_inner() {
            match p.as_rule() {
                Rule::doc_comment => result.doc_comment = Some(p.as_str().trim_end_matches(" \t")),
                Rule::uuid => result.uuid = Some(p.as_str()),
                Rule::other_attribute => result.attributes.push(p.as_str()),
                Rule::interface_name => result.name = p.as_str(),
                Rule::parent => result.parent = p.as_str(),
                Rule::method => result.methods.push(Method::from_pest(p)),
                Rule::typedef_enum => result.enums.push(TypedefEnum::from_pest(p)),
                Rule::typedef_struct => result.structs.push(TypedefStruct::from_pest(p)),
                _ => {}
            }
        }
        result
    }

    pub fn render(&self, w: &mut impl Write) -> io::Result<()> {
        write!(w, "{}", self.doc_comment.unwrap_or(""))?;
        if let Some(uuid) = self.uuid {
            writeln!(w, "#[com_interface(\"{}\")]", uuid)?;
        }
        writeln!(w, "pub trait {}: {} {{", self.name, self.parent)?;
        let mut first = true;
        for m in &self.methods {
            if first {
                first = false;
            } else {
                writeln!(w)?;
            }
            m.render(w)?;
        }
        writeln!(w, "}}")?;

        // Enums are top level.
        for e in &self.enums {
            writeln!(w)?;
            e.render(w)?;
        }

        for s in &self.structs {
            writeln!(w)?;
            s.render(w)?;
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
struct Document<'a> {
    interfaces: Vec<Interface<'a>>,
}

impl<'a> Document<'a> {
    fn from_pest(pair: Pair<'a, Rule>) -> Self {
        assert_eq!(pair.as_rule(), Rule::document);

        let mut result = Self::default();

        for p in pair.into_inner() {
            match p.as_rule() {
                Rule::interface => result.interfaces.push(Interface::from_pest(p)),
                _ => {}
            }
        }
        result
    }

    pub fn render(&self, w: &mut impl Write) -> io::Result<()> {
        let mut first = true;
        for i in &self.interfaces {
            if !first {
                writeln!(w)?;
            } else {
                first = false;
            }
            i.render(w)?;
        }
        Ok(())
    }
}

fn camel_to_snake(input: &str) -> String {
    let mut new = String::new();
    let mut seen_lowercase = false;

    for c in input.chars() {
        if c.is_uppercase() {
            if seen_lowercase {
                seen_lowercase = false;
                new.push_str("_");
            }
            new.push_str(&c.to_lowercase().to_string());
        } else if c == '_' {
            seen_lowercase = false;
            new.push(c);
        } else {
            seen_lowercase = true;
            new.push_str(&c.to_string())
        }
    }

    new
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut p = IDLParser::parse(Rule::document, &input).unwrap_or_else(|e| {
        eprintln!("Parsing error: {}", e);
        std::process::exit(1)
    });
    let doc = Document::from_pest(p.next().unwrap());
    print!(
        "{}",
        r#"#![allow(clippy::missing_safety_doc, non_camel_case_types, non_snake_case)]

// Generated by idl2rs.

use com::{com_interface, interfaces::{IUnknown, iunknown::IUnknownVTable}};
use winapi::shared::minwindef::{*, ULONG};
use winapi::shared::ntdef::*;
use winapi::shared::windef::*;
use winapi::shared::basetsd::*;
use winapi::um::oaidl::VARIANT;
use winapi::um::objidlbase::STATSTG;
use std::ffi::c_void;

#[repr(C)]
pub struct EventRegistrationToken {
    value: i64,
}

#[com_interface("0c733a30-2a1c-11ce-ade5-00aa0044773d")]
pub trait ISequentialStream: IUnknown {
    unsafe fn read(
        &self,
        pv: *mut c_void,
        cb: ULONG,
        pcbRead: *mut ULONG
    ) -> HRESULT;
    unsafe fn write(
        &self,
        pv: *const c_void,
        cb: ULONG,
        pcbWritten: *mut ULONG
    ) -> HRESULT;
}

#[com_interface("0000000c-0000-0000-C000-000000000046")]
pub trait IStream: ISequentialStream {
    unsafe fn seek(
        &self,
        dlibMove: LARGE_INTEGER,
        dwOrigin: DWORD,
        plibNewPosition: *mut ULARGE_INTEGER
    ) -> HRESULT;
    unsafe fn set_size(&self, libNewSize: ULARGE_INTEGER) -> HRESULT;
    unsafe fn copy_to(
        &self,
        pstm: *mut *mut IStreamVTable,
        cb: ULARGE_INTEGER,
        pcbRead: *mut ULARGE_INTEGER,
        pcbWritten: *mut ULARGE_INTEGER
    ) -> HRESULT;
    unsafe fn commit(&self, grfCommitFlags: DWORD) -> HRESULT;
    unsafe fn revert(&self) -> HRESULT;
    unsafe fn lock_region(
        &self,
        libOffset: ULARGE_INTEGER,
        cb: ULARGE_INTEGER,
        dwLockType: DWORD
    ) -> HRESULT;
    unsafe fn unlock_region(
        &self,
        libOffset: ULARGE_INTEGER,
        cb: ULARGE_INTEGER,
        dwLockType: DWORD
    ) -> HRESULT;
    unsafe fn stat(&self, pstatstg: *mut STATSTG, grfStatFlag: DWORD) -> HRESULT;
    unsafe fn clone(&self, ppstm: *mut *mut *mut IStreamVTable) -> HRESULT;
}


/// DLL export to create a WebView2 environment with a custom version of Edge,
/// user data directory and/or additional browser switches.
///
/// browserExecutableFolder is the relative path to the folder that
/// contains the embedded Edge. The embedded Edge can be obtained by
/// copying the version named folder of an installed Edge, like
/// 73.0.52.0 sub folder of an installed 73.0.52.0 Edge. The folder
/// should have msedge.exe, msedge.dll, etc.
/// Use null or empty string for browserExecutableFolder to create
/// WebView using Edge installed on the machine, in which case the
/// API will try to find a compatible version of Edge installed on the
/// machine according to the channel preference trying to find first
/// per user install and then per machine install.
///
/// The default channel search order is stable, beta, dev, and canary.
/// When there is an override WEBVIEW2_RELEASE_CHANNEL_PREFERENCE environment
/// variable or applicable releaseChannelPreference registry value
/// with the value of 1, the channel search order is reversed.
///
/// userDataFolder can be
/// specified to change the default user data folder location for
/// WebView2. The path can be an absolute file path or a relative file path
/// that is interpreted as relative to the current process's executable.
/// Otherwise, for UWP apps, the default user data folder will be
/// the app data folder for the package; for non-UWP apps,
/// the default user data folder `{Executable File Name}.WebView2`
/// will be created in the same directory next to the app executable.
/// WebView2 creation can fail if the executable is running in a directory
/// that the process doesn't have permission to create a new folder in.
/// The app is responsible to clean up its user data folder
/// when it is done.
///
/// additionalBrowserArguments can be specified to change the behavior of the
/// WebView. These will be passed to the browser process as part of
/// the command line. See
/// [Run Chromium with Flags](https://aka.ms/RunChromiumWithFlags)
/// for more information about command line switches to browser
/// process. If the app is launched with a command line switch
/// `--edge-webview-switches=xxx` the value of that switch (xxx in
/// the above example) will also be appended to the browser
/// process command line. Certain switches like `--user-data-dir` are
/// internal and important to WebView. Those switches will be
/// ignored even if specified. If the same switches are specified
/// multiple times, the last one wins. Note that this also applies
/// to switches like `--enable-features`. There is no attempt to
/// merge the different values of the same switch. App process's
/// command line `--edge-webview-switches` value are processed after
/// the additionalBrowserArguments parameter is processed.
/// Also note that as a browser process might be shared among
/// WebViews, the switches are not guaranteed to be applied except
/// for the first WebView that starts the browser process.
/// If parsing failed for the specified switches, they will be
/// ignored. `nullptr` will run browser process with no flags.
///
/// environment_created_handler is the handler result to the async operation
/// which will contain the WebView2Environment that got created.
///
/// The browserExecutableFolder, userDataFolder and additionalBrowserArguments
/// members of the environmentParams may be overridden by
/// values either specified in environment variables or in the registry.
///
/// When creating a WebView2Environment the following environment variables
/// are checked:
///
/// ```
/// WEBVIEW2_BROWSER_EXECUTABLE_FOLDER
/// WEBVIEW2_USER_DATA_FOLDER
/// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
/// WEBVIEW2_RELEASE_CHANNEL_PREFERENCE
/// ```
///
/// If an override environment variable is found then we use the
/// browserExecutableFolder, userDataFolder and additionalBrowserArguments
/// values as replacements for the corresponding values in
/// CreateCoreWebView2EnvironmentWithDetails parameters.
///
/// While not strictly overrides, there exists additional environment variables
/// that can be set:
///
/// ```
/// WEBVIEW2_WAIT_FOR_SCRIPT_DEBUGGER
/// ```
///
/// When found with a non-empty value, this indicates that the WebView is being
/// launched under a script debugger. In this case, the WebView will issue a
/// `Page.waitForDebugger` CDP command that will cause script execution inside the
/// WebView to pause on launch, until a debugger issues a corresponding
/// `Runtime.runIfWaitingForDebugger` CDP command to resume execution.
/// Note: There is no registry key equivalent of this environment variable.
///
/// ```
/// WEBVIEW2_PIPE_FOR_SCRIPT_DEBUGGER
/// ```
///
/// When found with a non-empty value, this indicates that the WebView is being
/// launched under a script debugger that also supports host applications that
/// use multiple WebViews. The value is used as the identifier for a named pipe
/// that will be opened and written to when a new WebView is created by the host
/// application. The payload will match that of the remote-debugging-port JSON
/// target and can be used by the external debugger to attach to a specific
/// WebView instance.
/// The format of the pipe created by the debugger should be:
/// `\\.\pipe\WebView2\Debugger\{app_name}\{pipe_name}`
/// where:
///
/// - `{app_name}` is the host application exe filename, e.g. WebView2Example.exe
/// - `{pipe_name}` is the value set for WEBVIEW2_PIPE_FOR_SCRIPT_DEBUGGER.
///
/// To enable debugging of the targets identified by the JSON you will also need
/// to set the WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS environment variable to
/// send `--remote-debugging-port={port_num}`
/// where:
///
/// - `{port_num}` is the port on which the CDP server will bind.
///
/// Be aware that setting both the WEBVIEW2_PIPE_FOR_SCRIPT_DEBUGGER and
/// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS environment variables will cause the
/// WebViews hosted in your application and their contents to be exposed to
/// 3rd party applications such as debuggers.
///
/// Note: There is no registry key equivalent of this environment variable.
///
/// If none of those environment variables exist, then the registry is examined next.
/// The following registry keys are checked:
///
/// ```
/// [{Root}\Software\Policies\Microsoft\EmbeddedBrowserWebView\LoaderOverride\{AppId}]
/// "releaseChannelPreference"=dword:00000000
/// "browserExecutableFolder"=""
/// "userDataFolder"=""
/// "additionalBrowserArguments"=""
/// ```
///
/// In the unlikely scenario where some instances of WebView are open during
/// a browser update we could end up blocking the deletion of old Edge browsers.
/// To avoid running out of disk space a new WebView creation will fail
/// with the next error if it detects that there are many old versions present.
///
/// ```
/// ERROR_DISK_FULL
/// ```
///
/// The default maximum number of Edge versions allowed is 20.
///
/// The maximum number of old Edge versions allowed can be overwritten with the value
/// of the following environment variable.
///
/// ```
/// WEBVIEW2_MAX_INSTANCES
/// ```
///
/// If the Webview depends on an installed Edge and it is uninstalled
/// any subsequent creation will fail with the next error
///
/// ```
/// ERROR_PRODUCT_UNINSTALLED
/// ```
///
/// First we check with Root as HKLM and then HKCU.
/// AppId is first set to the Application User Model ID of the caller's process,
/// then if there's no corresponding registry key the AppId is
/// set to the executable name of the caller's process, or if that
/// isn't a registry key then '*'. If an override registry key is found then we
/// use the browserExecutableFolder, userDataFolder and additionalBrowserArguments
/// registry values as replacements for the corresponding values in
/// CreateCoreWebView2EnvironmentWithDetails parameters. If any of those registry values
/// isn't present, then the parameter passed to CreateCoreWebView2Environment is used.
pub type FnCreateCoreWebView2EnvironmentWithDetails = unsafe extern "stdcall" fn(browserExecutableFolder: PCWSTR, userDataFolder: PCWSTR, additionalBrowserArguments: PCWSTR, environment_created_handler: *mut *mut ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandlerVTable) -> HRESULT;
"#
    );
    doc.render(&mut io::stdout()).unwrap();
}
