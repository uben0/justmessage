# TODO

- [ ] commands
  - [ ] enter - leave
  - [ ] pdf export
- [ ] group title
- [ ] check journald logs
- [ ] react to message
- [ ] tests

- [x] safe file write
- [x] bot for dev
- [x] auto save
- [x] month with languages
- [x] adapt word to singular or plural
- [x] infer minute past given time
- [x] infer day past given time
- [x] span day select
- [x] clear day
- [x] always give feedback on taken action
- [x] grammar token to upper case
- [x] display of date and time
- [x] instance based on group

# ROADMAP

- [ ] use normalized str for time zone
- [ ] admin console
- [ ] clippy
- [ ] rename bot
- [x] languages
- [x] enter then leave
- [x] gracefull exit
- [x] self-signed
- [ ] service
- [ ] security
  - [ ] encryption
  - [ ] limits
- [ ] telegram markdown

# COMMAND LIST

```
enter                 // adds a pending entry for right now
enter 18h30           // adds a pending entry for today at 18h30
leave                 // adds a span by using pending entry for right now
leave 21h15           // adds a span by using pending entry instant (today)
11h40 15h00           // adds a span today
tuesday 11h40 15h00   // adds a span last tuesday
24 11h40 15h00        // adds a span the 24th of the month
2025/09               // prints summary of september 2025
july                  // prints summary of july of this year
month                 // prints summary of this month
clear                 // removes all span from today
clear monday          // removes all span from last monday
```
