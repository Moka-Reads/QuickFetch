import requests
import json

def get_release_assets(owner, repo, tag):
    url = f"https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}"
    response = requests.get(url)
    data = json.loads(response.text)

    assets = data['assets']
    download_links = [asset['browser_download_url'] for asset in assets]

    return download_links

owner = "Mustafif"
repo = "MufiZ"
tag = "v0.6.0"

download_links = get_release_assets(owner, repo, tag)
print("vec![", end="")
for i, link in enumerate(download_links):
    if i != 0:
        print(", ", end="")
    print(f'"{link}"', end="")
print("]")
