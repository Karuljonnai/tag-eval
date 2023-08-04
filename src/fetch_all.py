#!/usr/bin/env python3

from requests import request as req
import sys, json, time

USER  = sys.argv[1]
TOKEN = sys.argv[2]

QUERIES = [
	'voteddown%3A',
	'votedup%3A',
	'fav%3A'
]

posts = []

for i in range(0, 3):
	for j in range(32): # 32 = limit the number of pages
		start = time.time()
		res = req(
			method='GET',
			url=f'https://e621.net/posts.json?tags={QUERIES[i]}{USER}&page={j}',
			auth=(USER, TOKEN),
			headers={
				'User-Agent': 'TagEval/1.0 (by Karuljonnai on e621)'
			}
		)
		raw = json.loads(res.text)
		if len(raw['posts']) == 0:
			break
		
		# delete unnecessary tags
		for post in raw['posts']:
			del post['created_at']
			del post['updated_at']
			del post['file']
			del post['preview']
			del post['sample']
			del post['score']
			del post['locked_tags']
			del post['change_seq']
			del post['flags']
			del post['rating']
			del post['fav_count']
			del post['sources']
			del post['pools']
			del post['relationships']
			del post['approver_id']
			del post['uploader_id']
			del post['description']
			del post['comment_count']
			del post['is_favorited']
			del post['has_notes']
			del post['duration']
			del post['tags']['copyright']
			del post['tags']['invalid']
			del post['tags']['lore']
			del post['tags']['meta']
			tags = []
			tags.extend(post['tags']['general'])
			tags.extend(post['tags']['species'])
			tags.extend(post['tags']['character'])
			tags.extend(post['tags']['artist'])
			del post['tags']
			post['tags']   = tags
			post['is_up']  = i == 1
			post['is_fav'] = i == 2
		posts.extend(raw['posts'])
		
		elapsed = time.time() - start
		if elapsed <= .5:
			time.sleep(.5 - elapsed)

with open('data/posts.json', 'w') as file:
	json_str = json.dumps(posts, separators=(',', ':'))
	sys.stdout.write(json_str)
	file.write(json_str)
