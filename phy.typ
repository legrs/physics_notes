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

#let resistor(l,p,q,r:(0,0,0))={
  import cetz.draw: *


  let (x1,y1) = p
  let (x2,y2) = q
  let (r1,r2,r3) = r
  let d1 = x2 -x1
  let d2 = y2 -y1
  let d_a = calc.sqrt(d1*d1 + d2*d2)
  let d_e1 = d1/d_a
  let d_e2 = d2/d_a

  let d_a7 = d_a/7

  let rot(a,b)={
    let (a1,a2) = a
    let (b1,b2) = b
    let x_ = b1*d_e1 - b2*d_e2
    let y_ = b1*d_e2 + b2*d_e1
    let a_2 = (a1+x_,a2+y_)
    return a_2
  }


    let pre = (x1,y1)
    let dif = (d_a7,-l)
    let a = rot(pre,dif)
  group({
    rotate(x:r1,y:r2,z:r3)

    line(pre,a)
    pre = a
      dif = (d_a7,2*l)
    a = rot(pre,dif)
    line(pre,a)
    pre = a
      dif = (d_a7,-2*l)
    a = rot(pre,dif)
    line(pre,a)
    pre = a
      dif = (d_a7,2*l)
    a = rot(pre,dif)
    line(pre,a)
    pre = a
      dif = (d_a7,-2*l)
    a = rot(pre,dif)
    line(pre,a)
    pre = a
      dif = (d_a7,2*l)
    a = rot(pre,dif)
    line(pre,a)
    pre = a
      dif = (d_a7,-l)
    a = rot(pre,dif)
    line(pre,a)
    pre = a
  })
}

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
#let spiral(p,q,s,n,r,c,t,doMark)={
  import cetz.draw: *
  let (p_x,p_y,p_z) = p
  let (q_x,q_y,q_z) = q
  let (s_x,s_y,s_z) = s
  let v_x = q_x/(2*calc.pi)
  let v_y = q_y/(2*calc.pi)
  let v_z = q_z/(2*calc.pi)
  let dtheta = 0.1
  let theta
  let update(theta,i)=(
      v_x * (theta + 2*i*calc.pi),
      v_y * (theta + 2*i*calc.pi),
      v_z * (theta + 2*i*calc.pi),
      v_x * (theta + dtheta + 2*i*calc.pi),
      v_y * (theta + dtheta + 2*i*calc.pi),
      v_z * (theta + dtheta + 2*i*calc.pi),
  )
  let x_1
  let y_1
  let z_1
  let x_2
  let y_2
  let z_2
  rotate(x:s_x,y:s_y,z:s_z)
  translate(p)
  for i in range(n) {
    let theta = 0
    (x_1,y_1,z_1,x_2,y_2,z_2) = update(theta,i)
    if doMark{
      line((x_1+ r * calc.cos(theta), y_1+ r * calc.sin(theta),z_1), (x_2+ r * calc.cos(theta+dtheta), y_2+ r * calc.sin(theta+dtheta),z_2),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
    }else{
      line((x_1+ r * calc.cos(theta), y_1+ r * calc.sin(theta),z_1), (x_2+ r * calc.cos(theta+dtheta), y_2+ r * calc.sin(theta+dtheta),z_2),stroke:(paint:c,thickness:t))
    }
    while (theta < 2*calc.pi) {
      (x_1,y_1,z_1,x_2,y_2,z_2) = update(theta,i)
      line((x_1+ r * calc.cos(theta), y_1+ r * calc.sin(theta), z_1),(x_2+ r * calc.cos(theta+dtheta), y_2+ r * calc.sin(theta+dtheta), z_2),stroke:(paint:c,thickness:t))
      theta += dtheta
    }
    (x_1,y_1,z_1,x_2,y_2,z_2) = update(theta,i)
    if doMark{
      line((x_1+ r * calc.cos(theta), y_1+ r * calc.sin(theta),z_1), (x_2+ r * calc.cos(theta+dtheta), y_2+ r * calc.sin(theta+dtheta),z_2),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
    }else{
      line((x_1+ r * calc.cos(theta), y_1+ r * calc.sin(theta),z_1), (x_2+ r * calc.cos(theta+dtheta), y_2+ r * calc.sin(theta+dtheta),z_2),stroke:(paint:c,thickness:t))
    }
    //if doMark{
    //  line((x+ r * calc.cos(theta), y+ r * calc.sin(theta),z+ v_x * (theta + 2*i*calc.pi)),(x+ r * calc.cos(theta+dtheta), y+ r * calc.sin(theta+dtheta),z+ v_x * (theta+dtheta+2*i*calc.pi)),mark:(end:">",fill:c),stroke:(paint:c,thickness:t))
    //}else{
    //  line((x+ r * calc.cos(theta), y+ r * calc.sin(theta),z+ v_x * (theta + 2*i*calc.pi)),(x+ r * calc.cos(theta+dtheta), y+ r * calc.sin(theta+dtheta),z+ v_x * (theta+dtheta+2*i*calc.pi)),stroke:(paint:c,thickness:t))
    //}
  }
  rotate(x:-s_x,y:-s_y,z:-s_z)
  translate((-p_x,-p_y,-p_z))
}

#let axis(len)={
  import cetz.draw: *
  line((0, 0, 0), (len, 0, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "x-axis")
  line((0, 0, 0), (0, len, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "y-axis")
  line((0, 0, 0), (0, 0, len), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "z-axis")
  content("x-axis.end", [$x$], anchor: "west")
  content("y-axis.end", [$y$], anchor: "south")
  content("z-axis.end", [$z$], anchor: "north-east")
}
