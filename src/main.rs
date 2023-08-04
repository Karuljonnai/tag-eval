mod profiler;

fn main() {
	#[allow(unused_mut)]
	let mut profile = profiler::Profile::new();
	profile.update_all(false);
	println!("Posts: {}\nTags: {}", profile.posts_len(), profile.tags_len());
}
