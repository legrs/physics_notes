/*
   Copyright 2026 Igarin & Legrs

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/

#import "@preview/physica:0.9.5": *
#import "@preview/unify:0.7.1": *
#import "@preview/cetz:0.4.2" as cetz

//#let lined(p1, p2, ..style) = {
//  let (x2, y2) = p2
//  let p2 = (x2 * calc.cos(y2) , x2 * calc.sin(y2))
//  draw.line(p1, p1 + p2, ..style)
//}

#let drawc(p,r,q,c,t)={
      import cetz.draw: *
  group({
    let (r_x,r_y,r_z) = q
    rotate(x:r_x,y:r_y,z:r_z)

    translate(p)
    circle((0,0,0),radius:r,stroke:(paint:c,thickness:t))
    line((0.1,-r,0),(-0.1,-r,0),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
    line((-0.1,r,0),(0.1,r,0),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
    line((r,0.1,0),(r,-0.1,0),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
    line((-r,-0.1,0),(-r,0.1,0),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
  })
}
#let drawc_t(r,c,t)={
  cetz.draw.circle((0,0,0),radius:r,stroke:(paint:c,thickness:t))
  cetz.draw.line((-0.1,-r,0),(0.1,-r,0),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
  cetz.draw.line((0.1,r,0),(-0.1,r,0),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
  cetz.draw.line((r,-0.1,0),(r,0.1,0),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
  cetz.draw.line((-r,0.1,0),(-r,-0.1,0),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
}
#let spiral(p,n,r,lpn,c,t,doMark)={
  import cetz.draw: *
  let (x,y,z) = p
  let v_x = lpn/(2*calc.pi)
  let dtheta = 0.1
  for i in range(n) {
    let theta = 0
    if doMark{
      line((x+ r * calc.cos(theta), y+ r * calc.sin(theta),z+ v_x * (theta + 2*i*calc.pi)), (x+ r * calc.cos(theta+dtheta), y+ r * calc.sin(theta+dtheta),z+ v_x * (theta+dtheta+2*i*calc.pi)),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
    }else{
      line((x+ r * calc.cos(theta), y+ r * calc.sin(theta),z+ v_x * (theta + 2*i*calc.pi)), (x+ r * calc.cos(theta+dtheta), y+ r * calc.sin(theta+dtheta),z+ v_x * (theta+dtheta+2*i*calc.pi)),stroke:(paint:c,thickness:t))
    }
    while (theta < 2*calc.pi) {
      line((x+ r * calc.cos(theta), y+ r * calc.sin(theta), z+ v_x * (theta + 2*i*calc.pi)),(x+ r * calc.cos(theta+dtheta), y+ r * calc.sin(theta+dtheta), z+ v_x * (theta+dtheta+2*i*calc.pi)),stroke:(paint:c,thickness:t))
      theta += dtheta
    }
    if doMark{
      line((x+ r * calc.cos(theta), y+ r * calc.sin(theta),z+ v_x * (theta + 2*i*calc.pi)),(x+ r * calc.cos(theta+dtheta), y+ r * calc.sin(theta+dtheta),z+ v_x * (theta+dtheta+2*i*calc.pi)),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
    }else{
      line((x+ r * calc.cos(theta), y+ r * calc.sin(theta),z+ v_x * (theta + 2*i*calc.pi)),(x+ r * calc.cos(theta+dtheta), y+ r * calc.sin(theta+dtheta),z+ v_x * (theta+dtheta+2*i*calc.pi)),stroke:(paint:c,thickness:t))
    }
  }
}
