use core::os::{copy_file, remove_file, path_exists};
use core::run::{program_output, ProgramOutput};

static SDL_PREFIX : &'static str = "/usr/local/lib";
static mergeFlag : &'static str = "-r";
static archiveCommand : &'static str = "ar";
static loadCommand : &'static str = "ld";
static outFlag : &'static str = "-o";
static aliasFlag : &'static str = "-alias";
static unexportedSymbolFlag : &'static str = "-unexported_symbol";

fn say(s: &str) {
    debug!("%s", s);
}

fn args_to_str(strs: &[~str]) -> ~str {
    str::connect(strs, ~" ")
}

fn make_writeable_by_user(file: &Path)
    -> (~str, ProgramOutput) {
    use core::libc::{S_IWUSR, S_IRUSR};

    let cmd = fmt!("chmod u+rw %s", file.to_str());
    (cmd, ProgramOutput{ status: (unsafe {
        str::as_c_str(file.to_str(), |buf| {
            let existing_permissions =
                file.stat().get().st_mode as int;
            libc::chmod(buf, (existing_permissions | S_IWUSR | S_IRUSR) as u16)
        })
    }) as int, out: ~"", err: ~""})
}

/// Possible flags for `ar`
enum ArchiveAction {
    Extract,
    Replace
}

impl ToStr for ArchiveAction {
    fn to_str(&self) -> ~str {
        match *self {
            Extract => ~"-x",
            Replace => ~"-r"
        }
    }
}

enum Reference {
    User,
    Group,
    World
}

enum Mode {
    Read,
    Write,
    Execute
}

/// Calls `ar`, returns a pair of the command string (for debugging) and the program output
fn archive_file(what: ArchiveAction, lib_out: &Path, obj_in: &Path) -> (~str, ProgramOutput) {
    let cmd = fmt!("ar %s %s %s", what.to_str(),
                   lib_out.to_str(),
                   obj_in.to_str());
    let p_output = program_output(archiveCommand, [what.to_str(),
                                                   lib_out.to_str(),
                                                   obj_in.to_str()]);
    (cmd, p_output)
}

/// Calls `ld`, returns a pair of the command string (for debugging) and the exit code
fn load_file(obj_in: &Path, obj_out: &Path, aliases: &[~str],
             unexported_symbols: &[~str]) -> (~str, ProgramOutput) {
    let cmd = fmt!("ld %s %s %s %s %s %s %s %s",
                   mergeFlag, obj_in.to_str(), outFlag.to_str(), obj_out.to_str(),
                   aliasFlag, args_to_str(aliases), unexportedSymbolFlag,
                   args_to_str(unexported_symbols));
    let p_output = program_output(loadCommand,
                   ~[mergeFlag.to_owned(),
                    obj_in.to_str(),
                    outFlag.to_owned(),
                    obj_out.to_str(),
                    ~"" + aliasFlag] +
                    aliases +
                    ~[unexportedSymbolFlag.to_owned()] +
                    unexported_symbols);
    (cmd, p_output)
    
}

/// Executes the action. If the result is nonzero, executes the error handler.
fn do_or(action: &fn() -> (~str, ProgramOutput),
         error_handler: &fn(&str, int) -> !) {
    let (cmd, program_output) = action();
    if program_output.status != 0 {
        io::println(fmt!("command was: %s [exit code %?]", cmd, program_output.status));
        io::println(fmt!("standard output was: %s", program_output.out));
        io::println(fmt!("standard error was: %s", program_output.err));
        error_handler(cmd, program_output.status);
    }
}

fn rename_file(old: &Path, new: &Path) -> (~str, ProgramOutput) {
    let cmd = fmt!("mv %s %s", old.to_str(), new.to_str());
    (cmd, ProgramOutput { status: str::as_c_str(old.to_str(), |old_c| {
       str::as_c_str(new.to_str(), |new_c| {
           unsafe { libc::rename(old_c, new_c) }
       })}) as int, out: ~"", err: ~""} )
}

#[pkg_do(post_build)]
fn post_build(build_dir: Path) {
    say(fmt!("Hello! I'm in: %s", os::getcwd().to_str()));
    let libsdlmain : Path = Path(SDL_PREFIX);
    let existing_sdl_lib_name : Path = libsdlmain.push("libSDLmain.a");
    let new_sdl_lib_name : Path = build_dir.push("libSDLXmain.a");
    let delete_new_sdl : &fn(&str, int) -> ! = |cmd, exit_code| {
        let _existed = remove_file(&new_sdl_lib_name);
        fail!(fmt!("System call %s failed, exit code was %?",
              cmd, exit_code));
    };
    let sdl_obj_file = build_dir.push("SDLMain.o");
    let sdlx_obj_file = build_dir.push("SDLXmain.o");
    do_or(|| { (fmt!("cp %s %s", existing_sdl_lib_name.to_str(), new_sdl_lib_name.to_str()),
                ProgramOutput { status: if copy_file(&existing_sdl_lib_name,
                                                     &new_sdl_lib_name) { 0 } else { 1 },
                               out: ~"", err: ~""})},
          |cmd, i| {
              io::println(fmt!("Copy command failed: %s", cmd));
              // *Really* fail
              unsafe { libc::exit(i as i32); }
          });
// work around copy_file permissions bug
    do_or(|| { make_writeable_by_user(&new_sdl_lib_name) }, delete_new_sdl);
    debug!("permissions of new file: %?",
               new_sdl_lib_name.stat().get().st_mode); // tjc: avoid get
    do_or(|| {
        archive_file(Extract, &new_sdl_lib_name, &sdl_obj_file)},
                          delete_new_sdl);
    do_or(|| {
        load_file(&sdl_obj_file, &sdlx_obj_file, ~[~"_main", ~"_SDLX_main"],
                  ~[~"main"])
    }, delete_new_sdl);
    do_or(|| { rename_file(&sdlx_obj_file, &sdl_obj_file) }, delete_new_sdl);
    do_or(|| { archive_file(Replace, &new_sdl_lib_name,
                            &sdl_obj_file) }, delete_new_sdl);
    do_or(|| { make_writeable_by_user(&new_sdl_lib_name) }, delete_new_sdl);
}

#[cfg(target_os="macos")]
#[pkg_do(configs)]
fn configs() {
    let usr_local = Path(fmt!("%s/libSDL.dylib", SDL_PREFIX));
    let usr_lib = Path(~"/usr/lib/libSDL.dylib");
    io::println(if path_exists(&usr_local) || path_exists(&usr_lib) {
        "mac_dylib"
    }
    else {
        "mac_framework"
    });
}

#[cfg(target_os="win32")]
#[cfg(target_os="freebsd")]
#[cfg(target_os="linux")]
fn configs() {}

// WRONG
// should not need this - rustpkg should handle checking the attributes
// and selecting out the right function
pub fn main() {
    let args = os::args();
    assert!(args.len() >= 3);
    debug!("args[0] = %s", args[0]);
    debug!("args[1] = %s", args[1]);
    debug!("args[2] = %s", args[2]);
    let build_dir = Path(args[0]).dir_path();
// first argument is root dir. we ignore it now, which we shouldn't
    if args[2] == ~"post_build" {
        post_build(build_dir);
    }
    else if args[2] == ~"configs" {
        configs();
    }
    else {
        io::println(fmt!("I only know how to do: \npost_build\nconfig \
                \nbut you wanted me to do: %s", args[2]));
        unsafe { libc::exit(1); }
    }
}