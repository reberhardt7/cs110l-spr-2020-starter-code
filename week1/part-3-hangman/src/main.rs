// Simple Hangman Program
// User gets five incorrect guesses
// Word chosen randomly from words.txt
// Inspiration from: https://doc.rust-lang.org/book/ch02-00-guessing-game-tutorial.html
// This assignment will introduce you to some fundamental syntax in Rust:
// - variable declaration
// - string manipulation
// - conditional statements
// - loops
// - vectors
// - files
// - user input
// We've tried to limit/hide Rust's quirks since we'll discuss those details
// more in depth in the coming lectures.
extern crate rand;
use rand::Rng;
use std::fs;
use std::io;
use std::io::Write;

const NUM_INCORRECT_GUESSES: u32 = 5;
const WORDS_PATH: &str = "words.txt";

fn pick_a_random_word() -> String {
    let file_string = fs::read_to_string(WORDS_PATH).expect("Unable to read file.");
    let words: Vec<&str> = file_string.split('\n').collect();
    String::from(words[rand::thread_rng().gen_range(0, words.len())].trim())
}

fn show_word_is_right(input: char, secret_word: &Vec<char>, show_word: &mut Vec<char>) -> bool {
    let mut is_right = false;
    for (n, &s) in secret_word.into_iter().enumerate() {
        if s == input {
            show_word[n] = input;
            is_right = true;
        }
    }

    is_right
}

fn main() {
    let secret_word = pick_a_random_word();
    // Note: given what you know about Rust so far, it's easier to pull characters out of a
    // vector than it is to pull them out of a string. You can get the ith character of
    // secret_word by doing secret_word_chars[i].
    let secret_word_chars: Vec<char> = secret_word.chars().collect();
    // Uncomment for debugging:
    println!("random word: {}", secret_word);

    let mut show_word = ['-'].repeat(secret_word.len());
    let mut left_chance = NUM_INCORRECT_GUESSES;
    let mut guess = Vec::new();

    println!(
        "The word so far is {}",
        show_word.iter().clone().collect::<String>()
    );
    println!("You have guessed the following letters:");
    println!("You have {} guess left", left_chance);
    print!("Please guess a letter: ");

    // Your code here! :)
    loop {
        io::stdout().flush().expect("Error flushing stdout.");
        let mut guess_char = String::new();
        io::stdin()
            .read_line(&mut guess_char)
            .expect("Error reading line.");

        let input_char = guess_char.chars().next().unwrap();
        guess.push(input_char);

        if !show_word_is_right(input_char, &secret_word_chars, &mut show_word) {
            println!("Sorry, that letter is not in the word");
            left_chance -= 1;
        }

        if left_chance > 0 && show_word == secret_word_chars {
            println!(
                "Congratulations you guessed the secret word: {}!",
                secret_word_chars.iter().clone().collect::<String>()
            );
            return;
        }

        if left_chance == 0 {
            println!("Sorry, you ran out of guesses!");
            return;
        }

        println!(
            "The word so far is {}",
            show_word.iter().clone().collect::<String>()
        );
        println!(
            "You have guessed the following letters: {}",
            guess.iter().clone().collect::<String>()
        );
        println!("You have {} guess left", left_chance);
        print!("Please guess a letter: ");
    }
}
