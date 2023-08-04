#![allow(dead_code)]

use std::{
	error::Error,
	fs::OpenOptions,
	io::prelude::*,
	process::Command,
	collections::HashMap,
};
use serde::{Serialize, Deserialize};
use serde_json as json;

#[derive(Serialize, Deserialize)]
struct Post {
	id :u32, // The first 2 bits are for storing the reaction (1st favourite; 2nd up-vote)
	tags :Vec<u32>
}
#[derive(Deserialize)]
struct RawPost {
	id     :u32,
	tags   :Vec<String>,
	is_up  :bool,
	is_fav :bool,
}
//TODO
pub struct EvalPost {
	id   :u32,
	tags :Vec<String>,
}

impl Post {
	#[inline]
	fn new(id :u32, tags :Vec<u32>, is_up :bool, is_fav :bool) -> Self {
		Self {
			id: id | (is_fav as u32 * 0x80000000) | (is_up as u32 * 0x40000000),
			tags
		}
	}
	#[inline]
	pub fn id(&self) -> u32 {
		self.id & 0x3FFFFFFF
	}
	pub fn factor(&self) -> i32 {
		const LUT :[i32; 4] = [-2, 1, 2, 3];
		LUT[(self.id >> 30) as usize]
	}
	fn mix(&mut self, post :Post) {
		self.id |= post.id & 0xC0000000;
	}
}

#[derive(Default)]
pub struct Profile {
	posts   :Vec<Post>,
	w_basic :Vec<i32>,
	//TODO w_bayes :[Vec<f32>; 2],
	tags    :HashMap<String, u32>,
	user    :String,
	token   :String,
}

impl Profile {
	pub fn new() -> Self {
		let user  = dotenv::var("USERNAME").expect("USERNAME or .env file not found!");
		let token = dotenv::var("API_TOKEN").expect("API_TOKEN or .env file not found!");
		Self {
			posts:   load_handler("data/posts.dat").unwrap_or_default(),
			w_basic: load_handler("data/w_basic.dat").unwrap_or_default(),
			//TODO w_bayes: load_handler("data/w_bayes.dat").unwrap_or_default(),
			tags:    load_handler("data/tags.dat").unwrap_or_default(),
			user,
			token
		}
	}
	pub fn update_all(&mut self, fetch_all :bool) {
		self.update_posts(fetch_all);
		self.update_weights();
	}
	pub fn update_posts(&mut self, fetch_all :bool) {
		let raw = if fetch_all {
			self.fetch_posts()
		} else {
			load_json()
		};
		self.posts.clear();
		
		for post in raw {
			let mut tags = Vec::<u32>::new();
			for tag in post.tags {
				tags.push(self.push_tag(tag));
			}
			self.push_post(Post::new(
				post.id,
				tags,
				post.is_up,
				post.is_fav
			));
		}
	}
	pub fn update_weights(&mut self) {
		self.update_w_basic();
	}
	fn update_w_basic(&mut self) {
		for e in self.w_basic.iter_mut() {*e = 0;}
		
		for post in &self.posts {
			let factor = post.factor();
			for tag in &post.tags {
				self.w_basic[*tag as usize] += factor;
			}
		}
	}
	pub fn save(&self) {
		save_handler("data/posts.dat",   &self.posts).expect("Should always save correctly.");
		save_handler("data/w_basic.dat", &self.w_basic).expect("Should always save correctly.");
		//TODO save_handler("data/w_bayes.dat", &self.w_bayes).expect("Should always save correctly.");
		save_handler("data/tags.dat",    &self.tags).expect("Should always save correctly.");
	}
	fn push_tag(&mut self, tag :String) -> u32 {
		if let Some(id) = self.tags.get(&tag) {
			return *id
		} else {
			let v = self.tags.len() as u32;
			self.tags.insert(tag, v);
			self.w_basic.push(0);
			return v;
		}
	}
	fn push_post(&mut self, post :Post) {
		for e in &mut self.posts {
			if post.id() == e.id() {
				e.mix(post);
				return;
			}
		}
		self.posts.push(post);
	}
	fn eval(&self, post :&Post) -> i32 {
		let mut score = 0;
		
		for i in &post.tags {
			score += self.w_basic.get(*i as usize).unwrap_or(&0);
		}
		
		score
	}
	fn fetch_posts(&self) -> Vec<RawPost> {
		let out = Command::new("python3")
			.args(["src/fetch_all.py", &self.user, &self.token])
			.output()
			.expect("Probable networking error during API fetching.")
			.stdout;
		
		json::from_slice(&out).expect("Program's output will always be valid JSON")
	}
	pub fn tags_basic_weights(&self) -> Vec<(i32, &str)> {
		let mut weights = Vec::<(i32, &str)>::new();
		
		for (name, id) in self.tags.iter() {
			weights.push((
				self.w_basic[*id as usize],
				name.as_str(),
			));
		}
		
		weights.sort_by(|a, b| {
			b.0.cmp(&a.0)
		});
		
		weights
	}
	#[inline]
	pub fn tags_len(&self) -> usize {
		self.tags.len()
	}
	#[inline]
	pub fn posts_len(&self) -> usize {
		self.posts.len()
	}
}

impl Drop for Profile {
	fn drop(&mut self) {
		self.save();
	}
}

fn load_json() -> Vec<RawPost> {
	let mut buf = String::new();
	OpenOptions::new()
		.read(true)
		.open("data/posts.json")
		.unwrap()
		.read_to_string(&mut buf)
		.unwrap();
	
	json::from_str(&buf).expect("File is always valid JSON")
}
fn load_handler<T>(path :&str) -> Result<T, Box<dyn Error>> where
	T: for<'a> Deserialize<'a> {
	let mut buf = Vec::<u8>::new();
	OpenOptions::new()
		.write(true)
		.create(true)
		.read(true)
		.open(path)
		.expect("`data` folder must exist for the program to run")
		.read_to_end(&mut buf)
		.expect("Nothing should make this fail; only if out of memory");
	
	let res :Result<T, _> = bincode::deserialize(buf.as_slice());
	
	return Ok(res?);
}
fn save_handler<T: Serialize>(path :&str, data :&T) -> Result<(), Box<dyn Error>> {
	let mut file = OpenOptions::new()
		.create(true)
		.write(true)
		.truncate(true)
		.open(path)?;
	
	file.write_all(bincode::serialize(data)?.as_slice())?;
	
	Ok(())
}
