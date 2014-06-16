#![crate_id = "chroot#1.0.0"]
/*
 * This file is part of the uutils coreutils package.
 *
 * (c) Vsevolod Velichko <torkvemada@sorokdva.net>
 *
 * For the full copyright and license information, please view the LICENSE
 * file that was distributed with this source code.
 */

#![feature(macro_rules)]
extern crate getopts;
extern crate libc;

use getopts::{optflag, optopt, getopts, usage};
use c_types::{get_pw_from_args, get_group};
use libc::funcs::posix88::unistd::{execvp, setuid, setgid};

#[path = "../common/util.rs"] mod util;
#[path = "../common/c_types.rs"] mod c_types;

extern {
    fn chroot(path: *libc::c_char) -> libc::c_int;
    fn setgroups(size: libc::c_int, list: *libc::gid_t) -> libc::c_int;
}

static NAME: &'static str = "chroot";
static VERSION: &'static str = "1.0.0";

#[allow(dead_code)]
fn main () { std::os::set_exit_status(uumain(std::os::args())); }

pub fn uumain(args: Vec<String>) -> int {
    let program = args.get(0);

    let options = [
        optopt("u", "user", "User (ID or name) to switch before running the program", "USER"),
        optopt("g", "group", "Group (ID or name) to switch to", "GROUP"),
        optopt("G", "groups", "Comma-separated list of groups to switch to", "GROUP1,GROUP2…"),
        optopt("", "userspec", "Colon-separated user and group to switch to. \
                                Same as -u USER -g GROUP. \
                                Userspec has higher preference than -u and/or -g", "USER:GROUP"),
        optflag("h", "help", "Show help"),
        optflag("V", "version", "Show program's version")
    ];

    let opts = match getopts(args.tail(), options) {
        Ok(m) => m,
        Err(f) => {
            show_error!("{}", f);
            help_menu(program.as_slice(), options);
            return 1
        }
    };

    if opts.opt_present("V") { version(); return 0 }
    if opts.opt_present("h") { help_menu(program.as_slice(), options); return 0 }

    if opts.free.len() == 0 {
        println!("Missing operand: NEWROOT");
        println!("Try `{:s} --help` for more information.", program.as_slice());
        return 1
    }

    let defaultShell: &'static str = "/bin/sh";
    let defaultOption: &'static str = "-i";
    let userShell = std::os::getenv("SHELL");

    let newroot = Path::new(opts.free.get(0).as_slice());
    if !newroot.is_dir() {
        crash!(1, "cannot change root directory to `{}`: no such directory", newroot.display());
    }

    let command: Vec<&str> = match opts.free.len() {
        1 => {
            let shell: &str = match userShell {
                None => defaultShell,
                Some(ref s) => s.as_slice()
            };
            vec!(shell, defaultOption)
        }
        _ => opts.free.slice(1, opts.free.len()).iter().map(|x| x.as_slice()).collect()
    };

    set_context(&newroot, &opts);

    unsafe {
        let executable = command.get(0).as_slice().to_c_str().unwrap();
        let mut commandParts: Vec<*i8> = command.iter().map(|x| x.to_c_str().unwrap()).collect();
        commandParts.push(std::ptr::null());
        execvp(executable as *libc::c_char, commandParts.as_ptr() as **libc::c_char) as int
    }
}

fn set_context(root: &Path, options: &getopts::Matches) {
    let userspecStr = options.opt_str("userspec");
    let userStr = options.opt_str("user").unwrap_or_default();
    let groupStr = options.opt_str("group").unwrap_or_default();
    let groupsStr = options.opt_str("groups").unwrap_or_default();
    let userspec = match userspecStr {
        Some(ref u) => {
            let s: Vec<&str> = u.as_slice().split(':').collect();
            if s.len() != 2 {
                crash!(1, "invalid userspec: `{:s}`", u.as_slice())
            };
            s
        }
        None => Vec::new()
    };
    let user = if userspec.is_empty() { userStr.as_slice() } else { userspec.get(0).as_slice() };
    let group = if userspec.is_empty() { groupStr.as_slice() } else { userspec.get(1).as_slice() };

    enter_chroot(root);

    set_groups(groupsStr.as_slice());
    set_main_group(group);
    set_user(user);
}

fn enter_chroot(root: &Path) {
    let rootStr = root.display();
    if !std::os::change_dir(root) {
        crash!(1, "cannot chdir to {}", rootStr)
    };
    let err = unsafe {
        chroot(".".to_c_str().unwrap() as *libc::c_char)
    };
    if err != 0 {
        crash!(1, "cannot chroot to {}: {:s}", rootStr, strerror(err).as_slice())
    };
}

fn set_main_group(group: &str) {
    if !group.is_empty() {
        let group_id = match get_group(group) {
            None => crash!(1, "no such group: {}", group),
            Some(g) => g.gr_gid
        };
        let err = unsafe { setgid(group_id) };
        if err != 0 {
            crash!(1, "cannot set gid to {:u}: {:s}", group_id, strerror(err).as_slice())
        }
    }
}

fn set_groups(groups: &str) {
    if !groups.is_empty() {
        let groupsVec: Vec<libc::gid_t> = FromIterator::from_iter(
            groups.split(',').map(
                |x| match get_group(x) {
                    None => crash!(1, "no such group: {}", x),
                    Some(g) => g.gr_gid
                })
            );
        let err = unsafe {
            setgroups(groupsVec.len() as libc::c_int,
                      groupsVec.as_slice().as_ptr() as *libc::gid_t)
        };
        if err != 0 {
            crash!(1, "cannot set groups: {:s}", strerror(err).as_slice())
        }
    }
}

fn set_user(user: &str) {
    if !user.is_empty() {
        let user_id = get_pw_from_args(&vec!(String::from_str(user))).unwrap().pw_uid;
        let err = unsafe { setuid(user_id as libc::uid_t) };
        if err != 0 {
            crash!(1, "cannot set user to {:s}: {:s}", user, strerror(err).as_slice())
        }
    }
}

fn strerror(errno: i32) -> String {
    unsafe {
        let err = libc::funcs::c95::string::strerror(errno);
        std::str::raw::from_c_str(err)
    }
}

fn version() {
    println!("{} v{}", NAME, VERSION)
}

fn help_menu(program: &str, options: &[getopts::OptGroup]) {
    version();
    println!("Usage:");
    println!("  {:s} [OPTION]… NEWROOT [COMMAND [ARG]…]", program);
    println!("");
    print!("{:s}", usage(
            "Run COMMAND with root directory set to NEWROOT.\n\
             If COMMAND is not specified, it defaults to '${SHELL} -i'. \
             If ${SHELL} is not set, /bin/sh is used.", options))
}
