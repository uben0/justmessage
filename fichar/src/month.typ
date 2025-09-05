#import "@preview/oxifmt:1.0.0": strfmt
#set page(width: auto, height: auto, margin: 1cm)

#let MONTHS = (
  en: (
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
  ),
  es: (
    [Enero],
    [Febrero],
    [Marso],
    [Abril],
    [Mayo],
    [Junio],
    [Julio],
    [Agosto],
    [Septiembre],
    [Octubro],
    [Noviembre],
    [Diciembre],
  )
);
#let WORDS = (
  en: (
    date: [date],
    enter: [enter],
    leave: [leave],
    duration: [duration],
    total: [Total],
  ),
  es: (
    date: [fecha],
    enter: [entra],
    leave: [sale],
    duration: [duración],
    total: [Total],
  ),
)

#let infos = json("month.json")
#let MONTHS = MONTHS.at(infos.language)
#let WORDS = WORDS.at(infos.language)

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
  MONTHS.at(month - 1)
}


#let hours-from-minutes(minutes) = {
  let hours = calc.div-euclid(minutes, 60)
  let minutes = calc.rem-euclid(minutes, 60)
  (hours: hours, minutes: minutes)
}

= #infos.year #fmt-month(infos.month)

== #infos.name

#table(columns: 4, align: (left, right, right, right),
  table.header(WORDS.date, WORDS.enter, WORDS.leave, WORDS.duration),
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

#WORDS.total: #fmt-duration(total)
