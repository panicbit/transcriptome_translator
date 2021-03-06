#![feature(plugin,type_macros,question_mark)]
#![plugin(phf_macros)]
extern crate phf;

#[macro_use]
extern crate nom;
extern crate scoped_threadpool;
extern crate monster;

mod codons;
mod translator;

use std::str;
use std::io;
use std::io::prelude::*;
use nom::{not_line_ending,line_ending};
use nom::IResult;
use std::sync::Arc;
use scoped_threadpool::Pool as ThreadPool;
use std::sync::mpsc::channel;
use std::fmt;

#[derive(Debug)]
struct Fasta<'a> {
    pub id: &'a str,
    pub sequence: Vec<&'a str>
}

pub fn start_parse<Output>(input: &[u8], mut output: Output, n_threads: u32) -> Result where
    Output: Write
{
    let mut threadpool = ThreadPool::new(n_threads);
    let (tx, rx) = channel();
    match fasta_deserialize(input){
        IResult::Done(_,o) => {
            threadpool.scoped(|threadpool| {
                for fasta in o {
                    let tx = tx.clone();
                    let amino_seq: Arc<Vec<u8>> = Arc::new(fasta.sequence
                        .iter()
                        .fold(Vec::new(), |mut acc, item| {
                                acc.extend(item.as_bytes());
                                acc
                        }));
                    let fasta_id = fasta.id;
                    let dispatch_decoding = |window, decoder: fn(&[u8]) -> String| {
                        let amino_seq = amino_seq.clone();
                        let tx = tx.clone();
                        threadpool.execute(move || {
                            tx.send(FastaComplete::new(window, fasta_id, decoder(&amino_seq))).expect("send");
                        });
                    };
                    dispatch_decoding("> No Move|", translator::no_move);
                    dispatch_decoding("> Shift Left One|", translator::nucleotide_shift_left_one);
                    dispatch_decoding("> Shift Left Two|", translator::nucleotide_shift_left_two);
                    dispatch_decoding("> Rev. No Move|", translator::rev_no_move);
                    dispatch_decoding("> Rev. Shift Left One|", translator::rev_nucleotide_shift_left_one);
                    dispatch_decoding("> Rev. Shift Left Two|", translator::rev_nucleotide_shift_left_two);
                }
            });

            drop(tx);

            for result in rx {
                output.write(result.window.as_bytes())?;
                output.write(result.id.as_bytes())?;
                output.write(result.sequence.as_bytes())?;
            }

            Ok(())
        }
        _ => Err(Error::Parsing)
    }
}

#[derive(Debug)]
struct FastaComplete<'a> {
    window: &'a str,
    id: &'a str,
    sequence: String,
}

impl<'a> FastaComplete<'a> {
    fn new(window: &'a str, id: &'a str, sequence: String) -> FastaComplete<'a> {
        FastaComplete {
            window: window,
            id: id,
            sequence: sequence
        }
    }
}

//FastaComplete.window are hardcoded to include labeling the id with '>
//and their reading frame.
//There is probably substantial room for improvement and the code could be deduplicated
//After memmapping the file.  I personally prefer laying it all out even if it does get
//a bit lengthy.
fn fasta_deserialize(input:&[u8]) -> IResult<&[u8], Vec<Fasta>>  {
    many0!(input,
      chain!(
        tag!(">") ~
        id: map_res!(not_line_ending, str::from_utf8) ~ line_ending ~
        sequence: many0!(terminated!(map_res!( is_not!(">\n"), str::from_utf8), tag!("\n"))),
        ||{
            Fasta {
                id: id,
                sequence: sequence
            }
        }
      )
   )
}

pub type Result = std::result::Result<(), Error>;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Parsing,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref e) => write!(f, "{}", e),
            Error::Parsing => write!(f, "Parsing failed")
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
