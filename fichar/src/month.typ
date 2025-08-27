#import "@preview/oxifmt:1.0.0": strfmt
#set page(width: auto, height: auto, margin: 1cm)

#let infos = json("month.json")

#let fmt-date((year, month, day)) = {
  strfmt("{:0>4}-{:0>2}-{:0>2}", year, month, day)
}
#let fmt-time((hour, minute, second)) = {
  strfmt("{:0>2}:{:0>2}", hour, minute)
}
#let fmt-duration((hours, minutes, seconds)) = {
  strfmt("{}h{:0>2}m", hours, minutes)
}
#let fmt-month(month) = {
  let months = (
    [January],
    [February],
    [Mars],
    [April],
    [May],
    [June],
    [Jully],
    [August],
    [September],
    [October],
    [November],
    [December],
  );
  months.at(month - 1)
}


#let seconds-hms(seconds) = {
  let minutes = calc.div-euclid(seconds, 60)
  let seconds = calc.rem-euclid(seconds, 60)
  let hours = calc.div-euclid(minutes, 60)
  let minutes = calc.rem-euclid(minutes, 60)
  (hours: hours, minutes: minutes, seconds: seconds)
}

= #infos.year #fmt-month(infos.month)

== #infos.name

#table(columns: 4,
  table.header([date], [enter], [leave], [duration]),
  .. infos.spans.map(
    span => (
      fmt-date(span.date),
      fmt-time(span.enters),
      fmt-time(span.leaves),
      fmt-duration(seconds-hms(span.seconds))
    ),
  ).flatten()
)

#let total = infos.spans.map(span => span.seconds).sum(default: 0)
#let total = seconds-hms(total)

Total: #fmt-duration(total)
