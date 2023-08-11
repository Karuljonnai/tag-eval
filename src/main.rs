mod profiler;
mod api;

use std::{
	io::{stdin, stdout, prelude::*},
	fs::{create_dir_all, OpenOptions}, collections::hash_map::DefaultHasher, hash::{Hash, Hasher}
};
use profiler::Profile;

fn main() {
	let mut profile = Profile::load().unwrap_or_else(|_| {
		println!("Failed to load previous profile, please answer the following questions to create one:");
		new()
	});
	let mut input = String::new();
	
	help();
	
	loop {
		print("> ");
		stdin().read_line(&mut input).unwrap_or_default();
		match *input.as_bytes().first().unwrap_or(&0) {
			b'n' => profile = new(),
			b'u' => profile.update(),
			b's' => search(&profile),
			b'c' => count(&profile),
			b'h' => help(),
			b'q' => break,
			_ => {
				println!("Incorrect command!");
				help();
			}
		}
		input.clear();
	}
	
	println!("Saving profile...");
	profile.save();
}

fn read() -> String {
	let mut buf = String::new();
	stdin().read_line(&mut buf).unwrap_or_default();
	buf.trim().to_owned()
}
#[inline]
pub fn print(message :&str) {
	print!("{}", message);
	stdout().flush().unwrap();
}
#[inline]
fn hash(text :&str) -> u64 {
	let mut hasher = DefaultHasher::default();
	text.hash(&mut hasher);
	hasher.finish()
}

#[inline]
fn help() {
	println!("Usage:
	new....: n
	update.: u
	search.: s
	count..: c
	help...: h
	quit...: q
	");
}
#[inline]
fn new() -> Profile {
	print("Enter your username:\t");
	let username = read();
	
	print("Enter your API key:\t");
	let api_token = read();
	
	Profile::new(&username, &api_token)
}
#[inline]
fn search(profile :&Profile) {
	print("Enter the tags:\t");
	let tags = read();
	
	let res = profile.search(&tags, None);
	create_dir_all("logs").expect("Program should have permission to create folders");
	
	let mut content = format!("Searched tags:\t{}\nlink\tscore\n", tags);
	for post in res {
		content.push_str(&format!("{}\n", post));
	}
	
	let name = format!("{:016x}", hash(&tags));
	OpenOptions::new()
		.write(true)
		.truncate(true)
		.create(true)
		.open(format!("logs/{}.log", name))
		.expect("Should be able to create files")
		.write_all(content.as_bytes())
		.expect("Should be able to write to files");
	
	println!("Successfully created search result at `logs/{}.log`", name);
}
#[inline]
fn count(profile :&Profile) {
	println!("Posts.: {}\nTags..: {}", profile.posts_len(), profile.tags_len());
}
