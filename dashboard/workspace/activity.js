// Bind each calendar once, after it has visible geometry. Preserve native scrolling.
function activityYears(weeks){
 const groups=[];
 weeks.forEach((week,index)=>{
  const year=week[0].day.slice(0,4),last=groups.at(-1);
  if(last?.year===year)last.end=index+1;else groups.push({year,start:index,end:index+1});
 });
 return '<div class="pa-activity-years" aria-label="Year"><span class="pa-activity-year-pin"><span></span></span>'+groups.map(g=>`<div class="pa-activity-year-segment" style="--weeks:${g.end-g.start}"><span class="pa-activity-year" data-year="${g.year}" data-start="${g.start}" data-end="${g.end}">${g.year}</span></div>`).join('')+'</div>';
}
function positionActivityYears(el,pitch){
 const left=el.scrollLeft,right=left+el.clientWidth,row=el.querySelector('.pa-activity-years');if(!row)return;
 const pin=row.querySelector('.pa-activity-year-pin>span'),labels=[...row.querySelectorAll('.pa-activity-year')];
 if(row.style.getPropertyValue('--activity-pitch')!==pitch+'px')row.style.setProperty('--activity-pitch',pitch+'px');
 const threshold=pin.offsetWidth+8;
 const current=labels.findLast(label=>Number(label.dataset.start)*pitch<=left+threshold)||labels[0];
 if(!current)return;
 if(pin.textContent!==current.textContent)pin.textContent=current.textContent;
 pin.dataset.year=current.dataset.year;
 for(const label of labels){
  const start=Number(label.dataset.start)*pitch,end=Number(label.dataset.end)*pitch;
  const hidden=Number(label.dataset.year)<=Number(current.dataset.year)||end<=left||start>=right;
  if(label.hidden!==hidden)label.hidden=hidden;
 }
 // Moving labels now travel with January in the native scrolling track. JS
 // only switches visibility at year boundaries; it never sets their position.
}

const activityGraphs=new Map();
const activityObserver=new ResizeObserver(entries=>{
 for(const {target} of entries)activityGraphs.get(target)?.layout();
});
function initActivityScroll(){
 for(const [el,info] of activityGraphs)if(!el.isConnected){
  activityObserver.unobserve(el);clearTimeout(info.timer);activityGraphs.delete(el);
 }
 for(const el of root.querySelectorAll('.pa-activity-graph')){
  if(activityGraphs.has(el)){activityGraphs.get(el).layout();continue;}
  const pane=el.closest('[data-pane]'),view=()=>activityView(pane?.dataset.pane);
  const info={ready:false,width:0,timer:null};
  const step=()=>{
   const weeks=el.querySelectorAll('.pa-activity-week');
   return weeks.length>1?weeks[1].offsetLeft-weeks[0].offsetLeft:19;
  };
  const save=()=>{
   if(!info.ready||!el.clientWidth)return;
   const v=view();if(!v)return;
   const pitch=step(),week=Math.floor(el.scrollLeft/pitch);
   positionActivityYears(el,pitch);
   v.activityScrollDay=activityShift(el.dataset.activityStart,week*7);
   v.activityScrollOffset=el.scrollLeft-week*pitch;
   const label=pane.querySelector('[data-activity-range]');
   const text=v.activityScrollDay+' — '+activityShift(v.activityScrollDay,Math.ceil(el.clientWidth/pitch)*7-1)+' · KST';
   if(label&&label.textContent!==text)label.textContent=text;
  };
  info.layout=()=>{
   const width=el.clientWidth,v=view();if(!width||!v)return;
   if(info.ready&&info.width===width){positionActivityYears(el,step());return;}
   const start=Date.parse(el.dataset.activityStart+'T12:00:00Z'),pitch=step();
   if(v.activityScrollDay){
    el.scrollLeft=(Date.parse(v.activityScrollDay+'T12:00:00Z')-start)/604800000*pitch+(v.activityScrollOffset||0);
   }else{
    const today=el.querySelector('[data-activity-day="'+activityDate(Date.now())+'"]');
    if(today){const a=today.getBoundingClientRect(),b=el.getBoundingClientRect();el.scrollLeft+=a.left-b.left-width*.7;}
   }
   info.ready=true;info.width=width;save();
  };
  const extend=()=>{
   if(!el.isConnected||!info.ready||!el.clientWidth)return;
   const left=el.scrollLeft<100,right=el.scrollLeft+el.clientWidth>el.scrollWidth-100;
   if(!left&&!right)return;
   const v=view();if(!v)return;save();
   v.activityWindowEnd=activityShift(v.activityWindowEnd||activityShift(activityDate(Date.now()),182),left?-364:364);
   const previous=state;let markup;
   try{state=v;markup=dashboardActivity();}finally{state=previous;}
   const template=document.createElement('template');template.innerHTML=markup;
   const next=template.content.querySelector('.pa-activity-graph');
   // Keep the scrolling element and all other panels mounted, including focus.
   const focusDay=el.contains(document.activeElement)?document.activeElement.dataset.activityDay:null;
   el.querySelector('.pa-activity-track').replaceWith(next.querySelector('.pa-activity-track'));
   el.dataset.activityStart=next.dataset.activityStart;
   const heading=el.closest('.pa-activity-calendar').querySelector('.pa-activity-heading small');
   patchResourceNode(heading,template.content.querySelector('.pa-activity-calendar .pa-activity-heading small'));
   info.ready=false;info.layout();
   if(focusDay)el.querySelector('[data-activity-day="'+focusDay+'"]')?.focus({preventScroll:true});
   persist();
  };
  el.addEventListener('scroll',()=>{
   if(!info.ready)return;
   save();clearTimeout(info.timer);info.timer=setTimeout(extend,180);
  },{passive:true});
  activityGraphs.set(el,info);activityObserver.observe(el);info.layout();
 }
}
new MutationObserver(records=>{
 if(records.some(r=>[...r.addedNodes,...r.removedNodes].some(n=>n.nodeType===1&&(n.matches?.('.pa-activity-graph')||n.querySelector?.('.pa-activity-graph')))))scheduleActivityLayout();
}).observe(root,{childList:true,subtree:true});
