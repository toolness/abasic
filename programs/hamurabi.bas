1 REM This is taken from page 79 of 101 BASIC Computer Games: Microcomputer Edition (1978):
2 REM https://archive.org/details/basic-computer-games-microcomputer-edition

10 print "Hamurabi"
20 print "Creative Computing  Morristown, New Jersey"
30 print:print:print
80 print "Try your hand at governing ancient Sumeria"
90 print "for a ten-year term of office.":print

91 REM *** VARIABLE INITIALIZATION
95 died=0:
96 pStarved=0: REM percent of population that starved per year on average
100 year=0: ppl=95:
101 s=2800: REM bushels in store
102 h=3000
103 e=h-s: REM # of bushels eaten by rats
110 bushelsPerAcre=3: acres=h/bushelsPerAcre: incoming=5: q=1
210 peopleStarved=0

215 print:print:print "Hamurabi: I beg to report to you,": year=year+1
217 print "in year "year", "peopleStarved" people starved, "incoming" came to the city."
218 ppl=ppl+incoming
227 if q > 0 then 230
228 ppl=int(ppl/2)
229 print "A horrible plague struck! Half the people died."
230 print "Population is now "ppl"."
232 print "The city now owns "acres" acres."
235 print "You harvested "bushelsPerAcre" bushels per acre."
250 print "Rats ate "e" bushels."
260 print "You now have "s" bushels in store.":print
270 if year=11 then 860
310 c=int(10*rnd(1)): bushelsPerAcre=c+17
312 print "Land is trading at "bushelsPerAcre" bushels per acre."
320 print "How many acres do you wish to buy";
321 input q: if q<0 then 850
322 if bushelsPerAcre*q<=s then 330
323 gosub 710
324 goto 320

330 if q=0 then 340
331 acres=acres+q: s=s-bushelsPerAcre*q: c=0
334 goto 400
340 print "How many acres do you wish to sell";
341 input q: if q<0 then 850
342 if q<acres then 350
343 gosub 720
344 goto 340
350 acres=acres-q: s=s+bushelsPerAcre*q: c=0
400 print
410 print "How many bushels do you wish to feed your people";
411 input q
412 if q<0 then 850

418 REM *** TRYING TO USE MORE GRAIN THAN IS IN SILOS?
420 if q <= s then 430
421 gosub 710
422 goto 410
430 s=s-q: c=1: print
440 print "How many acres do you wish to plant with seed";
441 input peopleStarved: if peopleStarved=0 then 511
442 if peopleStarved<0 then 850

444 REM *** TRYING TO PLANT MORE ACRES THAN YOU OWN?
445 if peopleStarved<=acres then 450
446 gosub 720
447 goto 440

449 REM *** ENOUGH GRAIN FOR SEED?
450 if int(peopleStarved/2)<=s then 455
452 gosub 710
453 goto 440

454 REM *** ENOUGH PEOPLE TO TEND THE CROPS?
455 if peopleStarved<10*ppl then 510
460 print "But you have only "ppl" people to tend the fields! Now then..."
470 goto 440
510 s=s-int(peopleStarved/2)
511 gosub 800

512 REM *** A BOUNTIFUL HARVEST!
515 bushelsPerAcre=c: h=peopleStarved*bushelsPerAcre: e=0
521 gosub 800
522 if int(c/2)<>c/2 then 530

523 REM *** RATS ARE RUNNING WILD!!
525 e=int(s/c)
530 s=s-e+h
531 gosub 800

532 REM *** LET US HAVE SOME BABIES
533 incoming=int(c*(20*acres+s)/ppl/100+1)

539 REM *** HOW MANY PEOPLE HAD FULL TUMMIES?
540 c=int(q/20)

541 REM *** HORROR, A 15% CHANCE OF PLAGUE
542 q=int(10*(2*rnd(1)-.3))
550 if ppl<c then 210

551 REM *** STARVE ENOUGH FOR IMPEACHMENT?
552 peopleStarved=ppl-c: if peopleStarved>.45*ppl then 560
553 pStarved=((year-1)*pStarved+peopleStarved*100/ppl)/year
555 ppl=c: died=died+peopleStarved: goto 215
560 print:print "You starved "peopleStarved" people in one year!"
565 print "Due to this extreme mismanagement you have not only"
566 print "been impeached and thrown out of office but you have"
567 print "also been declared national fink!!!!": goto 990

710 print "Hamurabi: Think again. You have only"
711 print s" bushels of grain. Now then..."
712 return

720 print "Hamurabi: Think again. You own only "acres" acres. Now then..."
730 return

800 c=int(rnd(1)*5)+1
801 return

849 REM *** DEALING WITH BAD INPUT
850 print:print "Hamurable: I cannot do what you wish."
855 print "Get yourself another steward!"
857 goto 990

860 print "In your 10-year term of office, "pStarved" percent of the"
862 print "population starved per year on the average, i.e. a total of"
865 print died;" people died!": l=acres/ppl
870 print "You started with 10 acres per person and ended with"
875 print l" acres per person.": print

880 if pStarved>33 then 565
885 if l<7 then 565
890 if pStarved>10 then 940
892 if l<9 then 940
895 if pStarved>3 then 960
896 if l<10 then 960

900 print "A fantastic performance!  Charlemagne, Disraeli, and"
905 print "Jefferson combined could not have done better!":goto 990

940 print "Your heavy-handed performance smacks of Nero and Ivan IV."
945 print "The people (remaining) find you an unpleasant ruler, and,"
950 print "frankly, hate your guts!":goto 990

960 print "Your performacne could have been somewhat better, but"
965 print "really wasn't too bad at all. " int(ppl*.8*rnd(1)) " people"
970 print "dearly like to see you assassinated but we all have our"
975 print "trivial problems."

990 REM The original code rang a bell 10 times here, we are not gonna do that.
995 print "So long for now.": print
999 end
