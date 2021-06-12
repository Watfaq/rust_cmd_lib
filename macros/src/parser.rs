use proc_macro2::TokenStream;
use quote::quote;
use std::iter::Peekable;
use ParseArg::*;

#[derive(Debug)]
pub enum ParseArg {
    ParsePipe,
    ParseSemicolon,
    ParseRedirectFd(i32, i32),                 // fd1, fd2
    ParseRedirectFile(i32, TokenStream, bool), // fd1, file, append?
    ParseArgStr(TokenStream),
    ParseArgVec(TokenStream),
}

pub struct Parser<I: Iterator<Item = ParseArg>> {
    iter: Peekable<I>,
}

impl<I: Iterator<Item = ParseArg>> Parser<I> {
    pub fn from(iter: Peekable<I>) -> Self {
        Self { iter }
    }

    pub fn parse(mut self, for_spawn: bool) -> TokenStream {
        let mut ret = quote!(::cmd_lib::GroupCmds::default());
        while self.iter.peek().is_some() {
            let cmd = self.parse_cmd();
            if !cmd.is_empty() {
                ret.extend(quote!(.append(#cmd)));
                if for_spawn && self.iter.peek().is_some() {
                    panic!("wrong spawning format: group command not allowed");
                }
            }
        }
        ret
    }

    fn parse_cmd(&mut self) -> TokenStream {
        let mut cmds = quote!(::cmd_lib::Cmds::default());
        while self.iter.peek().is_some() {
            let cmd = self.parse_pipe();
            cmds.extend(quote!(.pipe(#cmd)));
            if !matches!(self.iter.peek(), Some(ParsePipe)) {
                self.iter.next();
                break;
            }
            self.iter.next();
        }
        cmds
    }

    fn parse_pipe(&mut self) -> TokenStream {
        let mut ret = quote!(::cmd_lib::Cmd::default());
        while let Some(arg) = self.iter.peek() {
            match arg {
                ParseRedirectFd(fd1, fd2) => {
                    if fd1 != fd2 {
                        let mut redirect = quote!(::cmd_lib::Redirect);
                        match (fd1, fd2) {
                            (1, 2) => redirect.extend(quote!(::StdoutToStderr)),
                            (2, 1) => redirect.extend(quote!(::StderrToStdout)),
                            _ => panic!("unsupported fd numbers: {} {}", fd1, fd2),
                        }
                        ret.extend(quote!(.add_redirect(#redirect)));
                    }
                }
                ParseRedirectFile(fd1, file, append) => {
                    let mut redirect = quote!(::cmd_lib::Redirect);
                    match fd1 {
                        0 => redirect.extend(quote!(::FileToStdin(#file.into_path_buf()))),
                        1 => {
                            redirect.extend(quote!(::StdoutToFile(#file.into_path_buf(), #append)))
                        }
                        2 => {
                            redirect.extend(quote!(::StderrToFile(#file.into_path_buf(), #append)))
                        }
                        _ => panic!("unsupported fd ({}) redirect to file {}", fd1, file),
                    }
                    ret.extend(quote!(.add_redirect(#redirect)));
                }
                ParseArgStr(opt) => {
                    ret.extend(quote!(.add_arg(#opt.into_os_string())));
                }
                ParseArgVec(opts) => {
                    ret.extend(quote! (.add_args(#opts.iter().map(|s| ::std::ffi::OsString::from(s)).collect())));
                }
                ParsePipe | ParseSemicolon => break,
            }
            self.iter.next();
        }
        ret
    }
}
