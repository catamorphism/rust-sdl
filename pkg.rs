#[crate_type = "lib"]; // tjc: This should be inferred for pkgs scripts

use core::os::{copy_file, remove_file};
use core::run::program_output;

const SDL_PREFIX : &static/str = "/usr/local/lib";
const mergeFlag : &static/str = "-r";
const archiveCommand : &static/str = "ar";
const loadCommand : &static/str = "ld";
const outFlag : &static/str = "-o";
const aliasFlag : &static/str = "-alias";
const unexportedSymbolFlag : &static/str = "-unexported_symbol";

fn say(s: &str) {
    debug!("%s", s);
}

fn aliases_to_str(aliases: &[(~str, ~str)]) -> ~str {
    if aliases.is_empty() {
        return ~"";
    }
    let (from1, to1) = copy aliases[0];
    aliases.tail().foldr(fmt!("%s %s", from1, to1), |&(from, to), rest| {
        fmt!("%s %s %s ", from, to, rest)
    })
}

fn unexported_symbols_to_str(syms: &[~str]) -> ~str {
    if syms.is_empty() {
        return ~"";
    }
    let fst = copy syms[0];
    syms.tail().foldr::<~str>(fst, |s, rest| {
        fmt!("%s %s", *s, rest)
    })
}

fn to_numeric_mode(who: Reference, what: Mode) -> u16 {
    use core::libc::consts::os::posix88::{S_IRUSR, S_IWUSR, S_IXUSR};
    (match (who, what) {
        (User, Read) => S_IRUSR,
        (User, Write) => S_IWUSR,
        (User, Execute) => S_IXUSR,
        // WRONG -- tjc
        (Group, Read) => S_IRUSR,
        (Group, Write) => S_IWUSR,
        (Group, Execute) => S_IXUSR,
        (World, Read) => S_IRUSR,
        (World, Write) => S_IWUSR,
        (World, Execute) => S_IXUSR
    }) as u16
}

xxxxxxxx
 This is good, I just need to get the -L flag right
xxxxxxxx

fn add_file_permissions(who: Reference, what: Mode, file: &Path)
    -> (~str, int) {
    let cmd = fmt!("chmod 755 %s", file.to_str());
    (cmd, (unsafe {
        str::as_c_str(file.to_str(), |buf| {
            let existing_permissions =
                file.stat().get().st_mode;
            // !!!
            libc::chmod(buf, existing_permissions |
                        to_numeric_mode(who, what) )
        })
    }) as int)
}

/// Possible flags for `ar`
enum ArchiveAction {
    Extract
}

impl ToStr for ArchiveAction {
    pure fn to_str(&self) -> ~str {
        match *self {
            Extract => ~"-x"
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

/// Calls `ar`, returns the exit code
fn archive_file(what: ArchiveAction, lib_out: &Path, obj_in: &Path)
    -> (~str, int) {
    let cmd = fmt!("ar %s %s %s", what.to_str(),
                   lib_out.to_str(),
                   obj_in.to_str());
    match what {
        Extract => {
            // redirect program_output so we can do dry runs
            let p_output = program_output(archiveCommand, [what.to_str(),
                                                           lib_out.to_str(),
                                                           obj_in.to_str()]);
            debug!("ar error was: %s", copy p_output.err);
            (cmd, p_output.status)
        }
    }
}

/// Calls `ld`, returns the exit code
fn load_file(obj_in: &Path, obj_out: &Path, aliases: &[(~str, ~str)],
             unexported_symbols: &[~str]) -> (~str, int) {
// tjc: use out / err?
    let cmd = fmt!("ld %s %s %s %s %s %s %s %s",
                   mergeFlag, obj_in.to_str(), outFlag.to_str(), obj_out.to_str(),
                   aliasFlag, aliases_to_str(aliases), unexportedSymbolFlag,
                   unexported_symbols_to_str(unexported_symbols));
    let p_output = program_output(loadCommand,
                   [mergeFlag.to_owned(),
                    obj_in.to_str(),
                    outFlag.to_owned(),
                    obj_out.to_str(),
                    aliasFlag.to_owned(),
                    aliases_to_str(aliases),
                    unexportedSymbolFlag.to_owned()]);

/*,
                    unexported_symbols_to_str(unexported_symbols)])*/
    let error_output = copy p_output.err;
    debug!("load error: %s", error_output);
    (cmd, p_output.status)
    
}

/// Does the action. If the result is nonzero, does the error handler.
fn do_or(action: &fn() -> (~str, int),
         error_handler: &fn(&str, int) -> !) {
    let (cmd, result_code) = action();
// Should pass a whole ProgramOutput so we can print contents
// of err
    debug!("command was: %s [exit code %?]", cmd, result_code);
    if result_code != 0 {
        error_handler(cmd, result_code);
    }
}

fn rename_file(old: &Path, new: &Path) -> (~str, int) {
    let cmd = fmt!("mv %s %s", old.to_str(), new.to_str());
    (cmd, str::as_c_str(old.to_str(), |old_c| {
       str::as_c_str(new.to_str(), |new_c| {
           unsafe { libc::rename(old_c, new_c) }
        })}) as int)
}

#[pkg_do(post_build)]
fn post_build(build_dir: Path) {
    say(fmt!("Hello! I'm in: %s", os::getcwd().to_str()));
// tjc: delete new_sdl_lib_name if any commands fail
    let libsdlmain : Path = Path(SDL_PREFIX);
    let existing_sdl_lib_name : Path = libsdlmain.push("libSDLmain.a");
    let new_sdl_lib_name : Path = build_dir.push("libSDLXmain.a");
    let delete_new_sdl : &fn(&str, int) -> ! = |cmd, exit_code| {
//        let _existed = remove_file(&new_sdl_lib_name);
        // For debugging, don't delete it
        fail!(fmt!("System call %s failed, exit code was %?",
              cmd, exit_code));
    };
    let sdl_obj_file = build_dir.push("SDLMain.o");
    let sdlx_obj_file = build_dir.push("SDLXmain.o");
    do_or(|| { (fmt!("cp %s %s", existing_sdl_lib_name.to_str(), new_sdl_lib_name.to_str()),
                if copy_file(&existing_sdl_lib_name, &new_sdl_lib_name) { 0 } else { 1 })},
          |cmd, _i| { fail!(fmt!("Copy command failed: %s", cmd)) });
// work around copy_file permissions bug
    do_or(|| { add_file_permissions(User, Write, &new_sdl_lib_name) },
          delete_new_sdl);
    debug!("permissions of new file: %?",
               new_sdl_lib_name.stat().get().st_mode); // tjc: avoid get
    do_or(|| {
        archive_file(Extract, &new_sdl_lib_name, &sdl_obj_file)},
                          delete_new_sdl);             
    do_or(|| {
        load_file(&sdl_obj_file, &sdlx_obj_file, ~[(~"_main", ~"_SDLX_main")],
                  ~[(~"main")])
    }, delete_new_sdl);
    do_or(|| { rename_file(&sdlx_obj_file, &sdl_obj_file) }, delete_new_sdl);
    do_or(|| { archive_file(Extract, &new_sdl_lib_name,
                            &sdl_obj_file) }, delete_new_sdl);
    do_or(|| { add_file_permissions(User, Write, &new_sdl_lib_name) },
          delete_new_sdl);
}

// should not need this
fn main() {
    let args = os::args();
    fail_unless!(args.len() >= 3);
    debug!("args[0] = %s", args[0]);
    debug!("args[1] = %s", args[1]);
    debug!("args[2] = %s", args[2]);
    let build_dir = Path(args[0]).dir_path();
// first argument is root dir. we ignore it now, which we shouldn't
    if args[2] == ~"post_build" {
        post_build(build_dir);
    }
    else {
        fail!(fmt!("I only know how to do: post_build \
                \nbut you wanted me to do: %s", args[2]));
    }
}