# combine-latest

Combines two streams into a new stream which yields tuples with the latest values from each input
stream. Inspired by Rx's combineLatest.

```rust
use async_stream::stream;
use combine_latest::combine_latest;

fn combine_weather_data_streams(
    temperature: impl Stream<i32>,
    weather_notes: impl Stream<String>,
) -> Stream<String> {
    stream! {
        for await (t, n) in combine_latest(temperature, weather_notes) {
            yield format!("Temperature {t}°, note: {n}");
        }
    }
}
```

`combine_latest` won't yield its first tuple until both input streams have yielded a value. If you
need to get items as soon as the first is available, there is a `combine_latest_optional` function
that yields `(Option<T1>, Option<T2>)` tuples.

As values come in over time on the `temperature` and `weather_notes` streams, `combine_latest` and
`combine_latest_optional` will yield values like so:

|time| temperature  | weather_notes  | combine_latest         | combine_latest_optional            |
|----|--------------|----------------|------------------------|------------------------------------|
| 0  | 25           |                |                        | (Some(25), None)                   |
| 1  | 26           |                |                        | (Some(26), None)                   |
| 2  |              | Low visibility | (26, "Low visibility") | (Some(26), Some("Low visibility")) |
| 3  |              | Foggy          | (26, "Foggy")          | (Some(26), Some("Foggy"))          |
| 4  | 25           |                | (25, "Foggy")          | (Some(25), Some("Foggy"))          |


Since the same input value might be returned several times in the output stream, the items that the
input streams yield *must implement Clone.*

For types that don't implement Clone, it's possible to use the function `map_latest`:

```rust
fn combine_weather_data_streams(
    temperature: impl Stream<NonCloneTemperature>,
    weather_notes: impl Stream<String>,
) -> Stream<String> {
    stream! {
        for await output in map_latest(
            temperature,
            weather_notes,
            |t, n| format!("Temperature {t}°, note: {n}"),
        ) {
            yield output;
        }
    }
}

```

To add to you project: `cargo add combine-latest`
