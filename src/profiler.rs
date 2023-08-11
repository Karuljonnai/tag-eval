use std::{
	error::Error,
	fs::{OpenOptions, create_dir_all},
	io::prelude::*,
	collections::HashMap,
	cmp::Ordering,
};
use serde::{Serialize, Deserialize};
use crate::api::{RawPost, fetch};

#[derive(Serialize, Deserialize)]
struct ReactedPost {
	id   :u32,
	tags :Vec<u32>
}

struct Post {
	id   :u32,
	tags :Vec<u32>,
}

pub struct EvalPost {
	score :f32,
	post  :Post
}
impl std::fmt::Display for EvalPost {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "https://e621.net/posts/{:010}\t{:.6}", self.post.id, self.score)
	}
}

impl PartialEq for EvalPost {
	fn eq(&self, other: &Self) -> bool {
		self.score == other.score
	}
}
impl PartialOrd for EvalPost {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		if self.score > other.score {return Some(Ordering::Greater);}
		else if self.score < other.score {return Some(Ordering::Less);}
		Some(Ordering::Equal)
	}
}
impl Eq for EvalPost {}
impl Ord for EvalPost {
	fn cmp(&self, other: &Self) -> Ordering {
		self.partial_cmp(other).unwrap()
	}
}
impl EvalPost {
	fn vec_from(posts :Vec<Post>) -> Vec<Self> {
		let mut evals = Vec::with_capacity(posts.len());
		for post in posts {
			evals.push(Self {
				score: 1.0,
				post
			});
		}
		
		evals
	}
}

impl ReactedPost {
	#[inline]
	fn new(post :Post, is_fav :bool, is_up :bool) -> Self {
		Self {
			id:   post.id | (is_fav as u32 * 0x80000000) | (is_up as u32 * 0x40000000),
			tags: post.tags
		}
	}
	#[inline]
	pub fn id(&self) -> u32 {
		self.id & 0x3FFFFFFF
	}
	fn factor(&self) -> (u32, u32) {
		const LUT :[(u32, u32); 4] = [(0, 2), (1, 0), (2, 0), (3, 0)];
		LUT[(self.id >> 30) as usize]
	}
	#[inline]
	fn mix(&mut self, post :ReactedPost) {
		self.id |= post.id & 0xC0000000;
	}
}

#[derive(Serialize, Deserialize, Default)]
struct Bayes {
	freq :Vec<(u32, u32)>,
	prob :Vec<(f32, f32)>,
	sum  :(u64, u64),
	init :f32,
}

impl Bayes {
	fn sort(&self, posts :&mut [EvalPost]) {
		for mut post in &mut *posts {
			self.eval(&mut post);
		}
		
		posts.sort();
		posts.reverse();
	}
	#[inline]
	fn eval(&self, post :&mut EvalPost) -> f32 {
		post.score = self.init;
		
		for tag in &post.post.tags {
			let tmp = self.prob[*tag as usize];
			post.score += tmp.0 - tmp.1;
		}
		
		post.score
	}
	fn update(&mut self, posts :&[ReactedPost], len :usize) {
		self.freq.clear();
		self.sum = (len as u64, len as u64);
		self.extend(posts, len);
	}
	fn extend(&mut self, posts :&[ReactedPost], len :usize) {
		self.freq.resize(len, (1, 1));
		
		for post in posts {
			let factor = post.factor();
			for tag in &post.tags {
				self.freq[*tag as usize].0 += factor.0;
				self.freq[*tag as usize].1 += factor.1;
				self.sum.0 += factor.0 as u64;
				self.sum.1 += factor.1 as u64;
			}
		}
		
		self.update_prob(len);
	}
	fn update_prob(&mut self, len :usize) {
		let pos = self.sum.0 as f32;
		let neg = self.sum.1 as f32;
		let total = (self.sum.0 + self.sum.1) as f32;
		self.init = (pos / total).log2() - (neg / total).log2();
		
		self.prob.resize(len, (0.0, 0.0));
		
		for i in 0..len {
			self.prob[i] = (
				(self.freq[i].0 as f32 / pos).log2(),
				(self.freq[i].1 as f32 / neg).log2(),
			);
		}
	}
}

/// Contains all necessary user data to build a
/// [Content-Based Filtering Recommender System](https://en.wikipedia.org/wiki/Recommender_system#Content-based_filtering)
/// to predict, using the [Multinomial Naive Bayes Classifier algorithm](https://en.wikipedia.org/wiki/Naive_Bayes_classifier#Multinomial_naive_Bayes),
/// what new content the user would probably like the most.
#[derive(Default)]
pub struct Profile {
	posts :Vec<ReactedPost>,
	bayes :Bayes,
	tags  :HashMap<String, u32>,
	user  :String,
	token :String,
}

impl Profile {
	/// Creates a brand new [`Profile`].
	/// 
	/// # Notes
	/// 
	/// This function may take a long time to finish.
	pub fn new(username :&str, api_token :&str) -> Self {
		let mut profile = Self {
			posts: Vec::<ReactedPost>::default(),
			bayes: Bayes::default(),
			tags:  HashMap::<String, u32>::default(),
			user:  username.to_string(),
			token: api_token.to_string()
		};
		
		profile.update();
		profile.save();
		
		profile
	}
	/// Loads a previously created [`Profile`].
	/// 
	/// Requires that the `.env` and `data` folder exist.
	pub fn load() -> Result<Self, Box<dyn Error>> {
		let user  = dotenv::var("USERNAME")?;
		let token = dotenv::var("API_TOKEN")?;
		
		Ok(Self {
			posts: load_handler("data/posts.dat")?,
			bayes: load_handler("data/bayes.dat")?,
			tags:  load_handler("data/tags.dat")?,
			user,
			token
		})
	}
	/// Uses the API to fetch all posts that the user reacted, also updating
	/// the tags database in the process when it encounters a new tag.
	/// 
	/// It then resets and update the Bayesian profiler to fit this new data.
	/// 
	/// # Panics
	/// 
	/// - Cannot connect to the API.
	/// - Incorrect username or API key.
	/// 
	/// # Notes
	/// 
	/// This function may take a long time to finish.
	pub fn update(&mut self) {
		const QUERIES :&[(&str, bool, bool); 3] = &[
			("voteddown:", false, false),
			("votedup:", false, true),
			("fav:", true, false),
		];
		
		self.posts.clear();
		for query in QUERIES {
			let raws = fetch(
				&format!("{}{}", query.0, self.user),
				&self.user,
				&self.token,
				None,
			);
			
			for raw in raws {
				self.push_tags(&raw);
				self.push_post(ReactedPost::new(
					self.convert_raw(raw),
					query.1,
					query.2
				));
			}
		}
		
		self.bayes.update(&self.posts, self.tags_len());
	}
	/// Searches for posts using the [default tag search syntax](https://e621.net/wiki_pages/9169)
	/// and sorts the result based on the probability of the user liking them.
	pub fn search(&self, tags :&str, page_limit :Option<u8>) -> Vec<EvalPost> {
		let raws = fetch(
			&tags,
			&self.user,
			&self.token,
			page_limit
		);
		
		let mut posts = Vec::new();
		for raw in raws {
			posts.push(self.convert_raw(raw));
		}
		
		let mut evals = EvalPost::vec_from(posts);
		self.bayes.sort(&mut evals);
		
		evals
	}
	/// Saves to the `data` folder.
	/// 
	/// # Panics
	/// 
	/// - Doesn't have permission to crate and/or write files.
	#[inline]
	pub fn save(&self) {
		create_dir_all("data").expect("Program should have permission to create folder");
		
		let buf = format!("USERNAME={}\nAPI_TOKEN={}",self.user,self.token);
		OpenOptions::new()
			.write(true)
			.truncate(true)
			.create(true)
			.open(".env")
			.expect("Program should have permission to create and write files")
			.write_all(buf.as_bytes())
			.expect("Should be able to write to the file by this point");
		
		save_handler("data/posts.dat", &self.posts).expect("Should always save correctly.");
		save_handler("data/bayes.dat", &self.bayes).expect("Should always save correctly.");
		save_handler("data/tags.dat",  &self.tags ).expect("Should always save correctly.");
	}
	/// Returns how many tags the database currently holds.
	#[inline]
	pub fn tags_len(&self) -> usize {
		self.tags.len()
	}
	/// Returns how many reacted posts the database currently holds.
	#[inline]
	pub fn posts_len(&self) -> usize {
		self.posts.len()
	}
	fn push_tags(&mut self, post :&RawPost) {
		for tag in &post.tags {
			if self.tags.get(tag).is_none() {
				self.tags.insert(
					tag.clone(),
					self.tags.len() as u32
				);
			}
		}
	}
	fn push_post(&mut self, post :ReactedPost) {
		for e in &mut self.posts {
			if post.id() == e.id() {
				e.mix(post);
				return;
			}
		}
		self.posts.push(post);
	}
	fn convert_raw(&self, post :RawPost) -> Post {
		let mut tags = Vec::<u32>::with_capacity(post.tags.len());
		for tag in post.tags {
			if let Some(id) = self.tags.get(&tag) {
				tags.push(*id);
			}
		}
		
		Post {
			id: post.id,
			tags
		}
	}
}

fn load_handler<T>(path :&str) -> Result<T, Box<dyn Error>> where
	T: for<'a> Deserialize<'a> {
	let mut buf = Vec::<u8>::new();
	OpenOptions::new()
		.read(true)
		.open(path)?
		.read_to_end(&mut buf)
		.expect("Nothing should make this fail; only if out of memory");
	
	Ok(bincode::deserialize(buf.as_slice()).expect("Should not fail to deserialize"))
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
