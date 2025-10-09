# Rust Idioms and Patterns (Concise Guide)

A curated overview of common Rust idioms and patterns from the Rust Unofficial community.

---

## Ownership & Borrowing Idioms

### Borrowing Instead of Copying
Prefer borrowing (`&T`) over cloning/copying (`T`) when possible.

        fn print_len(s: &String) {
            println!("{}", s.len());
        }

### Avoiding `clone()` Unless Needed
Cloning is explicit and should signal cost. Use references instead where possible.

---

## Option & Result Idioms

### Early Returns with `?`
Propagate errors cleanly with the `?` operator.

        fn read_config(path: &str) -> Result<String, io::Error> {
            let content = fs::read_to_string(path)?;
            Ok(content)
        }

### Using `Option::map` and `Result::map`
Avoid verbose `match` by using combinators.

        let len = maybe_string.map(|s| s.len()).unwrap_or(0);

---

## Iterators

### Iterator Adapters
Prefer iterators over indexing or manual loops.

        let sum: i32 = numbers.iter().map(|x| x * 2).sum();

### Collecting Into Types
Leverage `.collect()` with turbofish for clarity.

        let set: HashSet<_> = vec![1,2,3].into_iter().collect();

---

## Error Handling

### Avoid Panicking in Libraries
Don’t use `unwrap()` or `expect()` in library code. Return `Result` instead.

### `thiserror` or Custom Error Enums
Use error enums to model domain-specific errors.

---

## Struct & API Design

### Builder Pattern
For complex structs, provide a builder to improve clarity.

        struct Config { debug: bool, port: u16 }

        impl Config {
            fn builder() -> ConfigBuilder {
                ConfigBuilder { debug: false, port: 8080 }
            }
        }

### Newtype Pattern
Wrap primitives for stronger typing and safety.

        struct UserId(u64);

---

## Concurrency

### `Send` + `Sync` with Threads
Leverage `Arc<Mutex<T>>` for shared ownership safely.

        let data = Arc::new(Mutex::new(0));

### Channels for Communication
Use `std::sync::mpsc` or `tokio::sync::mpsc` to send messages safely across threads.

---

## General Idioms

### `From` / `Into` for Conversions
Implement `From<T>` for clean type conversions.

        impl From<&str> for UserId {
            fn from(s: &str) -> Self { UserId(s.parse().unwrap()) }
        }

### RAII for Cleanup
Use destructors (`Drop`) to manage resources automatically.

        struct Guard;

        impl Drop for Guard {
            fn drop(&mut self) {
                println!("cleanup!");
            }
        }

### Pattern Matching
Use `match` exhaustively for clarity and safety.

        match option {
            Some(x) => println!("{}", x),
            None => println!("empty"),
        }

---

## Summary
- Prefer borrowing over cloning.  
- Use `?`, combinators, and iterators to simplify logic.  
- Avoid panics in library code; model errors explicitly.  
- Apply newtype, builder, and RAII patterns to improve design.  
- Use channels and safe concurrency primitives for multithreading.
