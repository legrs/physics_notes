#import "@preview/physica:0.9.5": *
#import "@preview/unify:0.7.1": *
#import "@preview/cetz:0.4.2": canvas, draw

#let lined(p1, p2, ..style) = {
  let (x2, y2) = p2
  let p2 = (x2 * calc.cos(y2) , x2 * calc.sin(y2))
  draw.line(p1, p1 + p2, ..style)
}

