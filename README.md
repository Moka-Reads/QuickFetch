![QuickFetch logo](QuickFetch.png)

## A Library to Fetch well Quickly...

> :warning: WORK IN PROGRESS AND NOT READY TO BE USED YET

This library is built to handle multiple requests within a `Client` (`reqwest` client which will handle it all under a Client Pool)
, cache the response results, and handle these in parallel and asynchronously. 

The goal is to be a one-stop shop for handling local package manager development to handle multiple 
packages with a local cache to easily update, get and remove the different responses.

## TODO 

- [ ] Convert `url` approach to an `Entry` struct that is a serialized structure that will
be deserialized as a key, which will handle being able to check modification, url, name, etc. 
