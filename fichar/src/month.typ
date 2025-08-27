#import "@preview/oxifmt:1.0.0": strfmt
#set page(width: auto, height: auto, margin: 1cm)

#let infos = json("month.json")

#let fmt-date((year, month, day)) = {
  strfmt("{:0>4}-{:0>2}-{:0>2}", year, month, day)
}
#let fmt-time((hour, minute)) = {
  strfmt("{:0>2}:{:0>2}", hour, minute)
}
#let fmt-duration((hours, minutes)) = {
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


#let hours-from-minutes(minutes) = {
  let hours = calc.div-euclid(minutes, 60)
  let minutes = calc.rem-euclid(minutes, 60)
  (hours: hours, minutes: minutes)
}

= #infos.year #fmt-month(infos.month)

== #infos.name

#table(columns: 4,
  table.header([date], [enter], [leave], [duration]),
  .. infos.spans.map(
    span => (
      fmt-date(span.date),
      fmt-time(span.enter),
      fmt-time(span.leave),
      fmt-duration(hours-from-minutes(span.minutes))
    ),
  ).flatten()
)

#let total = hours-from-minutes(infos.minutes)

Total: #fmt-duration(total)
