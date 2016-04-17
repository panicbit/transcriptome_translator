#![feature(plugin)]
#![feature(type_macros)]
#![plugin(phf_macros)]
extern crate phf;

#[macro_use]
extern crate nom;
extern crate memmap;
extern crate scoped_threadpool;

use std::str;
use std::io::prelude::*;
use nom::{not_line_ending,line_ending};
use nom::IResult;
use memmap::{Mmap, Protection};
use std::vec::*;
use std::fs::OpenOptions;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use std::thread;
use std::mem;
use scoped_threadpool::Pool as ThreadPool;
use std::sync::mpsc::channel;
use std::rc::Rc;

#[derive(Debug)]
pub struct FASTA<'a> {
    pub id: &'a str,
    pub sequence: Vec<&'a str>
}

static CODONS: phf::Map<&'static str, char> = phf_map! {
    //Alanine
    "GCA" => 'A',
    "GCG" => 'A',
    "GCC" => 'A',
    "GCT" => 'A',
    //Aspartic_Acid (D)
    //Asparagine (N)
    //Cysteine
    "TGT" => 'C',
    "TGC" => 'C',
    //Aspartic_Acid
    "GAC" => 'D',
    "GAT" => 'D',
    //Glutamic_Acid
    "GAA" => 'E',
    "GAG" => 'E',
    //Phenylalanine
    "TTT" => 'F',
    "TTC" => 'F',
    //Glycine
    "GGA" => 'G',
    "GGG" => 'G',
    "GGC" => 'G',
    "GGT" => 'G',
    //Histidine
    "CAC" => 'H',
    "CAT" => 'H',
    //Isoleucine
    "ATT" => 'I',
    "ATC" => 'I',
    "ATA" => 'I',
    //Leucine (L)
    "TTG" => 'L',
    "TTA" => 'L',
    "CTA" => 'L',
    "CTC" => 'L',
    "CTG" => 'L',
    "CTT" => 'L',
    //Lysine (K)
    "AAA" => 'K',
    "AAG" => 'K',
    //Methionine (M)
    "ATG" => 'M',
    //Asparagine (N)
    "AAT" => 'N',
    "AAC" => 'N',
    //Pyrrolysine (O) Special Stop Codon
    "UAG" => 'O',
    //Proline (P)
    "CCA" => 'P',
    "CCG" => 'P',
    "CCC" => 'P',
    "CCT" => 'P',
    //Glutamine (Q)
    "CAA" => 'Q',
    "CAG" => 'Q',
    //Arginine (R)
    "AGA" => 'R',
    "AGG" => 'R',
    "CGT" => 'R',
    "CGC" => 'R',
    "CGA" => 'R',
    "CGG" => 'R',
    //Serine (S)
    "AGT" => 'S',
    "AGC" => 'S',
    "TCT" => 'S',
    "TCC" => 'S',
    "TCA" => 'S',
    "TCG" => 'S',
    //Threonine (T)
    "ACA" => 'T',
    "ACG" => 'T',
    "ACC" => 'T',
    "ACT" => 'T',
    //Selenocysteine (U)
    "UGA" => 'U',
    //Valine (V)
    "GTA" => 'V',
    "GTG" => 'V',
    "GTC" => 'V',
    "GTT" => 'V',
    //Tryptophan (W)
    "TGG" => 'W',
    //Tyrosine (Y)
    "TAT" => 'Y',
    "TAC" => 'Y',
    //Stop Codons
    "TGA" => '*',
    "TAA" => '*',
    "TAG" => '*',
    //Glutamic Acid (E) or glutamine (Q) (Z)
    //X = any of the 13
    //translation stop (*)
    //gap of indeterminate length (-)
};

pub fn start_parse() {
    let file_mmap = Mmap::open_path("/home/dhc-user/transcriptome_translator/test/test-nucleo.FASTA", Protection::Read).unwrap();
    let bytes: &[u8] = unsafe {
        file_mmap.as_slice() };
//This mmap technique is extremely fast and extremely efficient on large datasets. +1 for memmap
    let mut file = OpenOptions::new().create(true).read(false).write(true).open("./results.txt").unwrap();
    let mut threadpool = ThreadPool::new(4);
    let (tx, rx) = channel();
    if let IResult::Done(_,o) = fasta_deserialize(bytes) {
        threadpool.scoped(|threadpool| {
            for fasta in o {
                let tx = tx.clone();
                threadpool.execute(move || {
                    let amino_seq: Vec<u8> = fasta.sequence
                        .into_iter()
                        .fold(Vec::new(), |mut acc, item| {
                                acc.extend(item.as_bytes());
                                acc
                        });
                    let mut vec: Vec<FASTA_Complete> = Vec::new();
                    let nomove = FASTA_Complete {
                        window: "> No Move|",
                        id: fasta.id,
                        sequence: no_move(amino_seq.clone())
                    };
                    let sl1 = FASTA_Complete {
                        window: "> Shift Left One|",
                        id: fasta.id,
                        sequence: nucleotide_shift_left_one(amino_seq.clone())
                    };
                    let sl2 = FASTA_Complete {
                        window: "> Shift Left Two|",
                        id: fasta.id,
                        sequence: nucleotide_shift_left_two(amino_seq.clone())
                    };
                    let rnm = FASTA_Complete {
                        window: "> Rev. No Move|",
                        id: fasta.id,
                        sequence: rev_no_move(amino_seq.clone())
                    };
                    let rsl1 = FASTA_Complete {
                        window: "> Rev. Shift Left One|",
                        id: fasta.id,
                        sequence: rev_nucleotide_shift_left_one(amino_seq.clone())
                    };
                    let rsl2 = FASTA_Complete {
                        window: "> Rev. Shift Left Two|",
                        id: fasta.id,
                        sequence: rev_nucleotide_shift_left_two(amino_seq.clone())
                    };
                    vec.push(nomove);
                    vec.push(sl1);
                    vec.push(sl2);
                    vec.push(rnm);
                    vec.push(rsl1);
                    vec.push(rsl2);
                    tx.send(vec).unwrap();
                });
            }
        });

        drop(tx);

        for results in rx {
            for results in results {
                file.write(results.window.as_bytes());
                file.write(results.id.as_bytes());
                file.write(results.sequence.as_bytes());
            }
        }
        file.sync_all();

    }
}

#[derive(Debug)]
pub struct FASTA_Complete<'a> {
    window: &'a str,
    id: &'a str,
    sequence: String,
}

impl<'a> FASTA_Complete<'a> {
    fn new() -> FASTA_Complete<'a> {
        let sequence = String::new();
        let window = "None";
        let id = "None";
        let FASTA_Complete = FASTA_Complete {
            window: window,
            id: id,
            sequence: sequence
        };
        FASTA_Complete
    }
}
//FASTA_Complete.window are hardcoded to include labeling the id with '>
//and their reading frame.
//There is probably substantial room for improvement and the code could be deduplicated
//After memmapping the file.  I personally prefer laying it all out even if it does get
//a bit lengthy.
pub fn rc_handler(read: FASTA) -> Rc<FASTA> {
    let amino_seq = Rc::new(read);
    amino_seq

}

pub fn rev_nucleotide_shift_left_two(amino_clone: Vec<u8>) -> String {
    // fn rev_nucleotide_shift_left_two does the following:
    // Reverses all elements in the Array.
    // Removes element at position '0'
    // Removes elemtne at position '0'
    // Then we check to see if the vector is a multiple of three
    // IF the vector is not a multiple of three we remove from the end
    // of the vector a single element then check until the vector is a multiple of three.
    // Then we convert our stream of nucleotides into groups of three
    // We then convert our groups of three into utf8 encoded String
    // We then use a phf to convert our utf8 encoded Strings into corrosponding
    // u8 Amino Acid encoding,
    // We then push the results of the Amino Acid encoding to a vector.
    // We push() a newline to the end of the String to assist with file encoding.
    let mut done = String::new();
    let mut amino_clone = amino_clone;
    done.push('\n');
    amino_clone.reverse();
    amino_clone.remove(0);
    amino_clone.remove(0);

    trim_and_map(&mut amino_clone, &mut done);

    done.push('\n');
    done
}

pub fn rev_nucleotide_shift_left_one(amino_clone: Vec<u8>) -> String {
    // fn rev_nucleotide_shift_left_one does the following:
    // Reverses all elements in the Array.
    // Removes element at position '0'
    // Then we check to see if the vector is a multiple of three
    // IF the vector is not a multiple of three we remove from the end
    // of the vector a single element then check until the vector is a multiple of three.
    // Then we convert our stream of nucleotides into groups of three
    // We then convert our groups of three into utf8 encoded String
    // We then use a phf to convert our utf8 encoded Strings into corrosponding
    // u8 Amino Acid encoding,
    // We then push the results of the Amino Acid encoding to a vector.
    // We push() a newline to the end of the String to assist with file encoding.
    let mut done = String::new();
    let mut amino_clone = amino_clone;
    done.push('\n');
    amino_clone.reverse();
    amino_clone.remove(0);

    trim_and_map(&mut amino_clone, &mut done);

    done.push('\n');
    done
}

pub fn rev_no_move(amino_clone: Vec<u8>) -> String {
    // fn rev_no_move does the following:
    // Reverses all elements in the Array.
    // Then we check to see if the vector is a multiple of three
    // IF the vector is not a multiple of three we remove from the end
    // of the vector a single element then check until the vector is a multiple of three.
    // Then we convert our stream of nucleotides into groups of three
    // We then convert our groups of three into utf8 encoded String
    // We then use a phf to convert our utf8 encoded Strings into corrosponding
    // u8 Amino Acid encoding,
    // We then push the results of the Amino Acid encoding to a vector.
    // We push() a newline to the end of the String to assist with file encoding.
    let mut done = String::new();
    let mut amino_clone = amino_clone;
    done.push('\n');
    amino_clone.reverse();

    trim_and_map(&mut amino_clone, &mut done);

    done.push('\n');
    done
}

pub fn nucleotide_shift_left_two(amino_clone: Vec<u8>) -> String {
    // fn nucleotide_shift_left_two does the following:
    // Removes element at position '0'
    // Removes elemtne at position '0'
    // Then we check to see if the vector is a multiple of three
    // IF the vector is not a multiple of three we remove from the end
    // of the vector a single element then check until the vector is a multiple of three.
    // Then we convert our stream of nucleotides into groups of three
    // We then convert our groups of three into utf8 encoded String
    // We then use a phf to convert our utf8 encoded Strings into corrosponding
    // u8 Amino Acid encoding,
    // We then push the results of the Amino Acid encoding to a vector.
    // We push() a newline to the end of the String to assist with file encoding.
    let mut done = String::new();
    let mut amino_clone = amino_clone;
    amino_clone.remove(0);
    amino_clone.remove(0);

    trim_and_map(&mut amino_clone, &mut done);

    done.push('\n');
    done
}

pub fn nucleotide_shift_left_one(amino_clone: Vec<u8>) -> String {
    // fn nucleotide_shift_left_one does the following:
    // Removes elemtne at position '0'
    // Then we check to see if the vector is a multiple of three
    // IF the vector is not a multiple of three we remove from the end
    // of the vector a single element then check until the vector is a multiple of three.
    // Then we convert our stream of nucleotides into groups of three
    // We then convert our groups of three into utf8 encoded String
    // We then use a phf to convert our utf8 encoded Strings into corrosponding
    // u8 Amino Acid encoding,
    // We then push the results of the Amino Acid encoding to a vector.
    // We push() a newline to the end of the String to assist with file encoding.
    let mut done = String::new();
    let mut amino_clone = amino_clone;
    done.push('\n');
    amino_clone.remove(0);

    trim_and_map(&mut amino_clone, &mut done);

    done.push('\n');
    done
}

pub fn no_move<'a>(amino_clone: Vec<u8>) -> String {
    // fn no_move does the following:
    // Then we check to see if the vector is a multiple of three
    // IF the vector is not a multiple of three we remove from the end
    // of the vector a single element then check until the vector is a multiple of three.
    // Then we convert our stream of nucleotides into groups of three
    // We then convert our groups of three into utf8 encoded String
    // We then use a phf to convert our utf8 encoded Strings into corrosponding
    // u8 Amino Acid encoding,
    // We then push the results of the Amino Acid encoding to a vector.
    // We push() a newline to the end of the String to assist with file encoding.
    let mut done = String::new();
    let mut amino_clone = amino_clone;
    done.push('\n');

    trim_and_map(&mut amino_clone, &mut done);

    done.push('\n');
    done
}

fn trim_and_map(amino_clone: &mut Vec<u8>, done: &mut String) {
    // Trim elements from the end until the length is a multiple of 3
    let waste = amino_clone.len() % 3;
    for _ in 0..waste {
        amino_clone.pop();
    }
    debug_assert!(amino_clone.len() % 3 == 0);

    while amino_clone.is_empty() == false {
        let mapped = amino_clone.drain(..3).collect::<Vec<u8>>();
        let mapped = String::from_utf8(mapped);
        for map in mapped {
            let mapped = CODONS.get(&*map);
            match mapped {
                Some(ref p) => done.push(**p),
                None => println!("Done!"),
            }
        }
    }
}

pub fn fasta_deserialize(input:&[u8]) -> IResult<&[u8], Vec<FASTA>>  {
    many0!(input,
      chain!(
        tag!(">") ~
        id: map_res!(not_line_ending, str::from_utf8) ~ line_ending ~
        sequence: many0!(terminated!(map_res!( is_not!(">\n"), str::from_utf8), tag!("\n"))),
        ||{
            FASTA {
                id: id,
                sequence: sequence
            }
        }
      )
   )
}
fn main() {
    start_parse();
}
