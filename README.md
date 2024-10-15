# QuickFetch

## A Library to Fetch well Quickly...

Developed by Mustafif Khan | MoKa Reads 2024

> :warning: WORK IN PROGRESS AND NOT READY TO BE USED FOR PRODUCTION YET

This library is built to handle multiple requests within a `Client` (`reqwest` client which will handle it all under a Client Pool)
, cache the response results, and handle these asynchronously.

The goal is to be a one-stop shop for handling local package manager development to handle multiple
packages with a local cache to easily update, get and remove the different responses.

## Customize your Approach

We allow for different kinds of customizations on how you interact with QuickFetch, such as how you're notified,
how you choose to handle the response, and how you'd like to fetch.

### Notify Methods

- `NotifyMethod::Log` - Logs the response to the console
- `NotifyMethod::ProgressBar`- A multiprogress bar
- `NotifyMethod::Silent`- No notifications

### Response Methods

- `ResponseMethod::Bytes`- Takes in the full response
- `ResponseMethod::Chunks`- Takes in the response in chunks
- `ResponseMethod::BytesStream`- Takes in the response as a stream

### Fetch Methods

- `FetchMethod::Async`- Fetches asynchronously
- `FetchMethod::Sync`- Fetches synchronously
- `FetchMethod::Watch`- Fetches by watching for modification on the config file asynchrously

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
