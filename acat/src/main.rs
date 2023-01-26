use fuzz_runner::nyx::aux_buffer;

use clap::{Arg, Command};

use std::fs::{OpenOptions};
use std::str;

extern crate colored;
use colored::*;


fn print_aux_buffer(aux_buffer: &aux_buffer::AuxBuffer, target_file: &String, show_header: bool, show_cap: bool, show_config: bool, show_result: bool, show_misc: bool, colored_output: bool){
    println!(
        "\n************** {} **************",
        target_file.green().bold()
    );

    colored::control::set_override(colored_output);


    if show_header{
        println!("\n=> {}", "HEADER".blue().bold());
        print!("{}", format!("{:#?}", aux_buffer.header).yellow());
    }

    if show_cap{
        println!("\n=> {}", "CAP".blue().bold());
        print!("{}", format!("{:#?}", aux_buffer.cap).yellow());
    }

    if show_config{
        println!("\n=> {}", "CONFIG".blue().bold());
        print!("{}", format!("{:#?}", aux_buffer.config).yellow());
    }

    if show_result{
        println!("\n=> {}", "RESULT".blue().bold());
        print!("{}", format!("{:#?}", aux_buffer.result).yellow());
    }

    if show_misc && aux_buffer.misc.len != 0 {
        println!("\n=> {}", "MISC".blue().bold());
        let len = aux_buffer.misc.len;
        println!("{}", str::from_utf8(&aux_buffer.misc.data[0..len as usize]).unwrap().red());
    }
}

fn main() {

    let matches = Command::new("acat")
        .about("Fancy tool to debug aux buffers!")
        .arg(
            Arg::new("target_file")
                .short('f')
                .long("target_file")
                .value_name("TARGET")
                .num_args(1)
                .help("specifies target file (aux buffer)"),
        )
        .arg(
            Arg::new("show_header")
                .long("show_header")
                .value_name("SHOW_HEADER")
                .required(false)
                .action(clap::ArgAction::SetTrue)
                .help("show header section"),
        )
        .arg(
            Arg::new("show_cap")
                .long("show_cap")
                .value_name("SHOW_CAP")
                .required(false)
                .action(clap::ArgAction::SetTrue)
                .help("show capabilities section"),
        )
        .arg(
            Arg::new("show_config")
                .long("show_config")
                .value_name("SHOW_CONFIG")
                .required(false)
                .action(clap::ArgAction::SetTrue)
                .help("show config section"),
        )
        .arg(
            Arg::new("ignore_result")
                .long("ignore_result")
                .value_name("IGNORE_RESULT")
                .required(false)
                .action(clap::ArgAction::SetTrue)
                .help("dont't show result section"),
        )
        .arg(
            Arg::new("show_misc")
                .long("show_misc")
                .value_name("SHOW_MISC")
                .required(false)
                .action(clap::ArgAction::SetTrue)
                .help("show misc section"),
        )
        .arg(
            Arg::new("show_all")
                .short('a')
                .long("show_all")
                .value_name("SHOW_ALL")
                .required(false)
                .action(clap::ArgAction::SetTrue)
                .help("show all sections"),
        )
        .arg(
            Arg::new("disable_color")
                .short('c')
                .long("disable_color")
                .value_name("SHOW_ALL")
                .required(false)
                .action(clap::ArgAction::SetTrue)
                .help("show all sections"),
        )
        //.setting(AppSettings::ArgRequiredElseHelp)
        .get_matches();

    let show_header: bool = matches.get_flag("show_header");
    let show_cap: bool = matches.get_flag("show_cap");
    let show_config: bool = matches.get_flag("show_config");
    let show_result: bool = !matches.get_flag("ignore_result");
    let show_misc: bool = matches.get_flag("show_misc");
    let colered_output: bool = !matches.get_flag("disable_color");



    let aux_buffer_file = matches.get_one::<String>("target_file").expect("file not found");
    let aux_shm_f = OpenOptions::new()
        .write(false)
        .read(true)
        .open(aux_buffer_file)
        .expect("couldn't open aux buffer file");
    let aux_buffer = aux_buffer::AuxBuffer::new_readonly(aux_shm_f, true);

    aux_buffer.validate_header().unwrap();

    if matches.get_flag("show_all"){
        print_aux_buffer(&aux_buffer, &aux_buffer_file.to_string(), true, true, true, true, true, colered_output);
    }
    else{
        print_aux_buffer(&aux_buffer, &aux_buffer_file.to_string(), show_header, show_cap, show_config, show_result, show_misc, colered_output);
    }
    
}
