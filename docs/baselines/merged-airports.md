# Paired airport source inventory

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Measured from the local surface-data import on base `c8eddbbe`, protocol 31.

Import pack: `menu-1791646312004028625.pack`. SHA-256:
`65e6454d6f0397d22308a4d606a0ba0b30479609cc87f346ec5b3adc4b50e84f`.

Implementation work, nr-apmerge. These are source placements, not evidence of retail gameplay.
84 pairs across 13 theaters. KURILE, TVIET and UKR have no pair.
All 331 base-theater runway records were examined. No additional unpaired
records lie within 20,000 ft of each other, and every numbered name suffix
belongs to one of the listed pairs. The merged base list has 247 fields.
Each row gives old airport list numbers and source origin X/Z in feet.
The heading column is the source placement heading. Before lengths are the
two gameplay runway objects in scene 2: STRIP5A and STRIP6A fly 90 degrees
from that source heading; STRIP3A and STRIP7A follow it. Additional drawn
strips are specified by the retained [type plans](../formats/redrawn-airports.md#the-plans).
Every merged field has one 12,000 ft by 200 ft runway on the base heading.
The source IDs remain stable: object ID is `0x40000000 + ordinal`.

| Theater | Field | Old airport IDs | Object ordinals | Source origins X/Z (ft) | Source heading | Types | Before primary lengths (ft) | Merged center X/Y/Z (ft) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| APA | Rio Hato | 11, 17 | 10, 21 | 1077477/887766; 1068509/887296 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8376 | 1072993.0, 0.0, 887531.0 |
| APA | Howard | 12, 18 | 11, 22 | 1208521/1084301; 1199655/1083852 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8274 | 1202040.0, 0.0, 1084077.0 |
| APA | Sherman | 13, 19 | 12, 23 | 1272828/1298208; 1272356/1307161 | 90 | STRIP5.OT, STRIP5A.OT | 7494, 8361 | 1272592.0, 0.0, 1304733.0 |
| APA | Tocumen | 14, 20 | 13, 24 | 1534932/1199961; 1534463/1208908 | 90 | STRIP5.OT, STRIP5A.OT | 7494, 8355 | 1534698.0, 0.0, 1206483.0 |
| BAL | Liepaja | 3, 33 | 2, 52 | 109608/1069032; 109605/1076926 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 109607.0, 0.0, 1072979.0 |
| BAL | Kaunas | 4, 31 | 3, 50 | 535589/356371; 535587/364317 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 531492.0, 0.0, 360344.0 |
| BAL | Vilnius | 5, 19 | 4, 20 | 774424/297974; 765463/297510 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8369 | 771992.0, 0.0, 297742.0 |
| BAL | Panevezys | 6, 32 | 5, 51 | 724013/692240; 724009/700200 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 719915.0, 0.0, 696220.0 |
| BAL | Riga | 7, 20 | 6, 21 | 635119/1084387; 626150/1083935 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8377 | 630635.0, 0.0, 1084161.0 |
| BAL | Daugavpils | 8, 34 | 7, 53 | 1174554/798766; 1174553/806713 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1172506.0, 0.0, 802740.0 |
| BAL | Kuressaare | 9, 24 | 8, 43 | 412707/1658872; 412707/1666795 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 408611.0, 0.0, 1662834.0 |
| BAL | Parnu | 10, 25 | 9, 44 | 715882/1658891; 715888/1666855 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 715885.0, 0.0, 1666969.0 |
| BAL | Vasalemma | 11, 21 | 10, 22 | 700714/1969092; 691804/1968619 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8318 | 694211.0, 0.0, 1968856.0 |
| BAL | Tartu | 12, 26 | 11, 45 | 1199128/1691661; 1199137/1699545 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1199133.0, 0.0, 1695603.0 |
| BAL | Gdov | 13, 27 | 12, 46 | 1403952/1839113; 1403967/1847051 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1403960.0, 0.0, 1843082.0 |
| BAL | Sol'sty | 14, 28 | 13, 47 | 1625137/1683489; 1625149/1691389 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1627191.0, 0.0, 1687439.0 |
| BAL | Pskov | 15, 22 | 14, 23 | 1487103/1477587; 1478142/1477120 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8369 | 1482623.0, 0.0, 1481450.0 |
| BAL | Ostrov | 16, 29 | 15, 48 | 1428522/1232922; 1428520/1240844 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1430569.0, 0.0, 1236883.0 |
| BAL | Vitebsk | 17, 23 | 16, 24 | 1814789/478154; 1805827/477688 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8370 | 1808260.0, 0.0, 477921.0 |
| BAL | Postavy | 18, 30 | 17, 49 | 1256482/462859; 1256479/470771 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1254433.0, 0.0, 466815.0 |
| CUB | San Julian | 3, 19 | 2, 23 | 363644/921531; 363647/929498 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 361598.0, 0.0, 925515.0 |
| CUB | San Antonio | 7, 20 | 6, 24 | 1018944/1241111; 1018899/1249061 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1016874.0, 0.0, 1245086.0 |
| CUB | Managua | 8, 21 | 7, 25 | 1152022/1241328; 1152488/1232373 | 270 | STRIP5.OT, STRIP5A.OT | 7494, 8363 | 1152255.0, 0.0, 1234803.0 |
| CUB | Cienfuegos | 11, 22 | 10, 26 | 1896519/912374; 1888592/912388 | 270 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1892556.0, 0.0, 912381.0 |
| CUB | Gerona | 12, 23 | 11, 27 | 879559/823096; 879073/832033 | 90 | STRIP5.OT, STRIP5A.OT | 7494, 8345 | 879316.0, 0.0, 829613.0 |
| EGY | Cairo | 4, 18 | 3, 48 | 193632/666528; 192768/676180 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 193200.0, 0.0, 671354.0 |
| EGY | Luxor | 5, 19 | 4, 49 | 521291/338827; 520395/348519 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 520843.0, 0.0, 343673.0 |
| EGY | Kibrit | 7, 20 | 6, 50 | 726176/715680; 725280/725280 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 725728.0, 0.0, 720480.0 |
| EGY | Bir Hasanah | 9, 21 | 8, 51 | 1021088/551849; 1020192/561539 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1020640.0, 768.0, 556694.0 |
| EGY | El-Nakab | 11, 22 | 10, 52 | 1307648/486319; 1306755/496009 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1307202.0, 768.0, 491164.0 |
| EGY | Al Arish | 13, 23 | 12, 53 | 1086528/928678; 1085632/938352 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1086080.0, 0.0, 933515.0 |
| EGY | Hatzor | 16, 24 | 15, 54 | 1389631/879534; 1388735/889225 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1389183.0, 0.0, 884380.0 |
| EGY | Hatserim | 17, 25 | 16, 55 | 1299567/1043371; 1298671/1053040 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1299119.0, 0.0, 1048206.0 |
| FRA | Southampton | 14, 24 | 13, 101 | 274623/1428430; 265728/1427966 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8303 | 268128.0, 0.0, 1428198.0 |
| FRA | Antwerpe | 15, 25 | 14, 102 | 1249460/1477584; 1240484/1477056 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8384 | 1242924.0, 0.0, 1477320.0 |
| FRA | Brussels | 16, 26 | 15, 103 | 1241280/1371103; 1232352/1370623 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8336 | 1236816.0, 0.0, 1370863.0 |
| FRA | Chievres | 17, 27 | 16, 104 | 1044672/1248212; 1035776/1247732 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8304 | 1038176.0, 0.0, 1247972.0 |
| FRA | Troyes | 18, 28 | 17, 105 | 1011890/486358; 1003026/485878 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8272 | 1007458.0, 0.0, 486118.0 |
| FRA | Reims | 19, 29 | 18, 106 | 1044628/789496; 1035764/789016 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8272 | 1038148.0, 0.0, 789256.0 |
| FRA | Paris | 20, 30 | 19, 107 | 725152/682976; 716256/682496 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8304 | 718656.0, 0.0, 682736.0 |
| FRA | Beauvais | 21, 31 | 20, 108 | 585876/814052; 576980/813572 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8304 | 581428.0, 0.0, 813812.0 |
| FRA | Cambrai | 22, 32 | 21, 109 | 806976/1051616; 798304/1051136 | 0 | STRIP5.OT, STRIP5A.OT | 7494, 8080 | 802640.0, 512.0, 1051376.0 |
| GRE | Athinai | 1, 13 | 0, 41 | 815228/1076289; 807257/1076297 | 270 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 809195.0, 0.0, 1074245.0 |
| GRE | Skiros | 3, 14 | 2, 51 | 947106/1503418; 947112/1495466 | 180 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 943013.0, 0.0, 1501490.0 |
| GRE | Iraklion | 9, 15 | 14, 89 | 1141841/233274; 1141816/241223 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1139781.0, 0.0, 235201.0 |
| GRE | Izmir | 11, 16 | 19, 118 | 1617580/1471427; 1625530/1471440 | 90 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1621555.0, 0.0, 1467338.0 |
| GRE | Balikesir | 12, 17 | 22, 139 | 1797201/1773394; 1797208/1781331 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1795157.0, 0.0, 1777363.0 |
| IRA | King Khalid | 1, 14 | 2, 58 | 750714/521345; 741027/520475 | 270 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 745871.0, 0.0, 518862.0 |
| IRA | Basrah | 2, 21 | 3, 65 | 1045622/1504380; 1035935/1503495 | 270 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1040779.0, 0.0, 1501890.0 |
| IRA | Ali Al Salem | 3, 16 | 4, 60 | 1018772/1045603; 1019647/1035914 | 180 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1015114.0, 0.0, 1038711.0 |
| IRA | Aghajari | 4, 17 | 5, 61 | 1717209/1512244; 1707561/1511360 | 270 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1712385.0, 0.0, 1513850.0 |
| IRA | Ahvaz | 5, 18 | 6, 62 | 1504408/1805213; 1503512/1814909 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1499864.0, 0.0, 1810061.0 |
| IRA | Al Nasiriyah | 6, 19 | 7, 63 | 430969/1575827; 430073/1585517 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 430521.0, 0.0, 1580672.0 |
| IRA | Jalibah | 7, 20 | 8, 64 | 756596/1436829; 766274/1437709 | 90 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 759387.0, 0.0, 1435221.0 |
| IRA | Ahmed Al Jaber | 13, 15 | 14, 59 | 1100776/789439; 1110447/790330 | 90 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1107660.0, 0.0, 791933.0 |
| LFA | Stanley | 1, 5 | 0, 62 | 1602433/1101361; 1602436/1093426 | 180 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1602435.0, 0.0, 1099442.0 |
| NSK | Sunan | 2, 20 | 1, 21 | 553865/1666813; 544914/1667463 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14611 | 549390.0, 0.0, 1667138.0 |
| NSK | Onchon | 3, 21 | 2, 22 | 389920/1407743; 381022/1408376 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14558 | 387519.0, 0.0, 1408060.0 |
| NSK | Wonsan | 8, 22 | 7, 23 | 1215602/1569798; 1224480/1569146 | 180 | STRIP6.OT, STRIP6A.OT | 6763, 14538 | 1220041.0, 0.0, 1569472.0 |
| NSK | Kimpo | 9, 19 | 8, 20 | 848791/855038; 839867/855670 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14584 | 844329.0, 0.0, 855354.0 |
| NSK | Seoul | 10, 18 | 9, 19 | 1101106/725192; 1110043/724544 | 180 | STRIP6.OT, STRIP6A.OT | 6763, 14597 | 1101479.0, 0.0, 724868.0 |
| NSK | Choongwon | 12, 17 | 11, 18 | 1379403/593736; 1379405/601685 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1381452.0, 0.0, 597711.0 |
| PGU | Sharjah | 1, 17 | 8, 78 | 854977/494457; 864684/495338 | 90 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 863927.0, 0.0, 492850.0 |
| PGU | Ras Al Khaimah | 2, 18 | 9, 79 | 986025/676947; 986888/667323 | 180 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 984409.0, 0.0, 672135.0 |
| PGU | Fujairah | 3, 19 | 10, 80 | 1248231/389339; 1257908/390222 | 90 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1253070.0, 0.0, 387733.0 |
| PGU | Bandar Lengeh | 4, 21 | 11, 82 | 592794/986035; 602487/986924 | 90 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 597641.0, 0.0, 984432.0 |
| PGU | Bandar Abbas | 11, 20 | 18, 81 | 1209441/1406059; 1199757/1405181 | 270 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1204599.0, 0.0, 1401524.0 |
| SPA | Jodhpur | 6, 20 | 5, 24 | 1799375/658319; 1798473/668003 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1798924.0, 0.0, 663161.0 |
| SPA | Bikaner | 7, 21 | 6, 25 | 1668212/1209485; 1658551/1208611 | 270 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1661334.0, 0.0, 1209048.0 |
| SPA | Multan | 10, 19 | 9, 23 | 1569948/1739686; 1569057/1749354 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 1571551.0, 0.0, 1742472.0 |
| SPA | Hyderabad | 11, 17 | 10, 21 | 685171/543610; 684291/553299 | 0 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 682683.0, 0.0, 548455.0 |
| SPA | Sukkur | 12, 18 | 11, 22 | 658304/1641377; 667990/1642257 | 90 | STRIP3.OT, STRIP3A.OT | 7516, 8100 | 665195.0, 0.0, 1639769.0 |
| VLA | Jixian | 2, 20 | 1, 49 | 267120/609816; 258213/610488 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14567 | 262667.0, 0.0, 610152.0 |
| VLA | Pogranichnyy | 5, 21 | 4, 50 | 660320/1437184; 651412/1437854 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14568 | 655866.0, 512.0, 1437519.0 |
| VLA | Spassk Dalniy | 6, 22 | 5, 51 | 1028948/1518948; 1020036/1519620 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14572 | 1024492.0, 0.0, 1519284.0 |
| VLA | Vozdvizhenka | 8, 23 | 7, 52 | 799552/1199712; 790674/1200363 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14538 | 795113.0, 0.0, 1200038.0 |
| VLA | Petrovka | 9, 24 | 8, 53 | 889696/1027680; 880802/1028339 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14554 | 885249.0, 0.0, 1028010.0 |
| VLA | Unashi | 10, 25 | 9, 54 | 1069841/790255; 1060995/790907 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14506 | 1065418.0, 0.0, 790581.0 |
| VLA | Mudanjiang | 14, 26 | 13, 55 | 95086/1363562; 86185/1364240 | 0 | STRIP6.OT, STRIP6A.OT | 6763, 14561 | 90636.0, 512.0, 1363901.0 |
| WTA | Longtian | 7, 12 | 6, 21 | 920657/1707830; 920633/1715767 | 0 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 916549.0, 0.0, 1713847.0 |
| WTA | Tai-Nan | 8, 13 | 7, 22 | 1307563/601838; 1307573/593904 | 180 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1305520.0, 0.0, 597871.0 |
| WTA | Anpu | 9, 15 | 8, 24 | 1840056/1584858; 1840033/1576937 | 180 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1837997.0, 0.0, 1580898.0 |
| WTA | Chiang Kai Shek | 10, 16 | 9, 25 | 1707854/1602459; 1715822/1602465 | 90 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1711838.0, 0.0, 1606558.0 |
| WTA | Hua-Lien | 11, 14 | 10, 23 | 1831858/1011916; 1831850/1003986 | 180 | STRIP7.OT, STRIP7A.OT | 7388, 7948 | 1827758.0, 0.0, 1007951.0 |

## Consumers before merging

Both halves enter the airport scene as separate airports and runway objects.
AI home selection, tower service, navigation and ground-start lists can use either
half when its side and runway length qualify. AI taxi and parking points are
separate. Multiplayer mission starts store runway object IDs. Quick Mission
profile drafts store positions in the side-filtered airport list.

Ground-target templates are independent placements. The surface placement rules
use nearby source runway footprints to anchor airfield objectives and rank home
fields. Their own runway pieces are distinct from these theater pairs.

Recordings store the airfield scene version and source identities; checkpoints
store live runway references and airfield sequences. They require explicit
compatibility handling when the layout changes.

## Validation

Implementation validation on Linux against the identified import:

- `--airfield-audit --merged`: 84 fields, 0 problems. The output identifies
  source tile pictures and verifies flatness, bounds, water, centerline
  obstruction, four ground-start slots, pavement routes and old-ID lookup.
- The full `--airfield-audit`: 75 layouts, 1,028 runway objects, 0 problems.
- `--airfield-audit --parked`: 108 templates, 0 problems.
- `ai-merged-airfield-{f18,c130}-wing{2,4}`: all four flights pass. The
  scripted leader departs, and every AI wingman takes off, lands and parks
  with zero off-pavement ground samples. Hornets use FRA 14/24 and C-130s
  VLA 6/22. Each old second-half selection resolves to the merged field.
- Changed battery: 239/240 pass. The radar offscreen-render case cannot
  compile depth-texture loads on the available software OpenGL backend;
  the same error occurs in the pre-change executable.
- Quick guard against `334a70e7`: 40 same, 12 different, none missing.
  Nine recordings differ only by the intentional airfield-scene marker
  2 to 3. The dead-leader text and recording, and the AAA-threat recording,
  contain differences already present on starting base `c8eddbbe`.
  Re-running those two cases with the preserved pre-change executable gives
  identical stdout and identical non-header data chunks to this branch;
  only the new scene marker differs.

The local report `.local/next-round/reports/nr-apmerge.md` lists all guard
items and test evidence. Eight inspected CPU before/after views are indexed
by `.local/next-round/renders/nr-apmerge/README.md`. They cover Southampton
and Spassk Dalniy from above and from an approach angle. They use actual
terrain/static buffers and indexed art but omit GPU lighting, city fill,
weather and moving objects. Images remain local. Live network and
cross-platform execution were not validated in this socket-restricted
Linux environment. No running retail comparison was made.
