![QuickFetch logo](QuickFetch.png)

## A Library to Fetch well Quickly...

> :warning: WORK IN PROGRESS AND NOT READY TO BE USED YET

This library is built to handle multiple requests within a `Client` (`reqwest` client which will handle it all under a Client Pool)
, cache the response results, and handle these in parallel and asynchronously. 

The goal is to be a one-stop shop for handling local package manager development to handle multiple 
packages with a local cache to easily update, get and remove the different responses.

## Progress

- [X] Set an `Entry` trait to be used as the key for the `db` cache and responsible for the `Fetcher<E: Entry>` structure. 
- [X] Set different methods of handling the response data in the `Fetcher` structure, such as: 
  - [X] `Bytes` for storing the whole response as bytes
  - [X] `Chunks` for storing the response in chunks
  - [X] `BytesStream` for storing the response in a stream of bytes
- [X] Enable basic support for encryption and decryption of the response data using the `Entry` as the key. 
- [X] Provide `Config` and `Package` as a minimal package 
- [X] Provide `GithubPackage` to handle packages that can be downloaded from Github Releases
  - This is done using the `Package::github_release` method
