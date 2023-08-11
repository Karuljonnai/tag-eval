use std::{
	thread::sleep,
	time::{SystemTime, Duration}, io::{stdout, Write}
};
use reqwest::blocking::ClientBuilder;
use serde::Deserialize;
use serde_json as json;

#[derive(Deserialize)]
struct CrudePost {
	id   :u32,
	tags :Tags
}
impl CrudePost {
	#[inline]
	fn tags_len(&self) -> usize {
		self.tags.general.len()
		+ self.tags.species.len()
		+ self.tags.character.len()
		+ self.tags.artist.len()
	}
	fn to_raw(mut self) -> RawPost {
		let mut tags = Vec::with_capacity(self.tags_len());
		
		tags.append(&mut self.tags.general);
		tags.append(&mut self.tags.species);
		tags.append(&mut self.tags.character);
		tags.append(&mut self.tags.artist);
		
		RawPost {
			id: self.id,
			tags
		}
	}
}

#[derive(Deserialize)]
struct Tags {
	general   :Vec<String>,
	species   :Vec<String>,
	character :Vec<String>,
	artist    :Vec<String>,
}

#[derive(Deserialize)]
struct Response {
	posts :Vec<CrudePost>
}

pub(crate) struct RawPost {
	pub id   :u32,
	pub tags :Vec<String>,
}

impl RawPost {
	fn from_crudes(res :Response) -> Vec<RawPost> {
		let posts = res.posts;
		let mut raw = Vec::with_capacity(posts.len());
		
		for post in posts {
			raw.push(post.to_raw());
		}
		
		raw
	}
}

pub(crate) fn fetch(query :&str, username :&str, api_token :&str, page_limit :Option<u8>) -> Vec<RawPost> {
	let client = ClientBuilder::new()
		.user_agent("TagEval/1.0 (by Karuljonnai on e621)")
		.build()
		.expect("Not inside an async function");
	let mut posts = Vec::new();
	
	let page_limit = page_limit.unwrap_or(32);
	for p in 1..=page_limit {
		print!("Fetching page {:03} (Limit: {})", p, page_limit);
		stdout().flush().unwrap();
		let start = SystemTime::now();
		
		let res = client.get("https://e621.net/posts.json")
			.basic_auth(username, Some(api_token))
			.query(&[
				("tags", query),
				("page", &p.to_string())
			])
			.send()
			.expect("Username and API token should be correct; Program should have access to the internet; Request to the API will not redirect")
			.text()
			.expect("Successful response will always be valid UTF-8");
		println!(" Done");
		
		let mut raw = RawPost::from_crudes(json::from_str(&res).expect("Response will always be valid JSON"));
		
		if raw.is_empty() {break;}
		posts.append(&mut raw);
		
		const LIMIT :Duration = Duration::from_millis(500);
		let elapsed = start.elapsed().unwrap();
		if elapsed < LIMIT {sleep(LIMIT - elapsed);}
	}
	
	posts
}