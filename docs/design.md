# Station Iapetus (working title, subject to change)

The game is a sci-fi DeadSpace-like horror game, the action takes place on the spaceship that stays radio-silent near
Saturn. The main character had crashed in the loading bay, he's the only survivor, he has no other option but to
start exploring the main ship to find a way to escape (this is the main goal).

## Production plan

Since production resources are very limited, some of 3D models should be bought. Levels have to be modelled manually,
using gray-boxing to create overall spaceship structure and only after tests are over, levels should be filled with
details. This should help to significantly reduce costs of production.

Overall budget for 3D models is around 1000$ (can be increased if needed, but it should be less 2000$). It is also 
possible to use royalty-free 3D models, however their quality is questionable and such models should be used for
prototyping only. 

Since main mechanics are pretty much done, I expect production to be ended at in the late Spring of 2022. However since
the engine is somewhat incomplete, the actual plan can be modified.

**WEEKLY PLAN:**

- **29.11.21 - 5.12.21** Loading bay grayboxing + filling with gameplay elements
- **6.12.21 - 12.12.21** Medical deck grayboxing + filling with gameplay elements
- **13.12.21 - 19.12.21** Research deck grayboxing (maybe extend to 2 weeks?) + filling with gameplay elements
- **20.12.21 - 26.12.21** Sewage treatment plant grayboxing + filling with gameplay elements
- **27.12.21 - 02.01.22** Bridge grayboxing + filling with gameplay elements
- **03.01.22 - 09.01.22** Hydroponics and food storage grayboxing + filling with gameplay elements
- TBD

## Plot

The spaceship is classified, high-security level research facility that creates new experimental types of weapons,
including new highly-contagious viruses for Outer Planets Government. 

The ship is designed to be invisible for any kind of locators, it works in cycles, one cycle lasts for 1 year. During
this period,the engines are disabled, radio transmitters are off and the ship hides on the dark side of various 
satellites of Saturn and uses compressed air engines to remain in the shadow. Personnel don't have any radio transmitters,
every device is "offline" and connected only to local network. The ship equipped with few high-power railguns to destroy
any possible threat which can work either in automatic or manual mode.

Once the cycle is over, Outer Planets Government (OPG) sends another ship to restore supplies and take the results of 
experiments. Once resupplied, the ship flies to random satellite and the cycle starts over.

There are two groups of scientists working simultaneously in different sections of research deck, each group consists
of 50 people, thus there are 100 scientists working on the ship at the same time.

In total, there is around 1500 of people on the ship, they're working in various places, starting from the kitchen at
each deck and ending by the bridge, where is the high command is located.

One group of scientists (called Alpha) were working on a new virus that erasing the previous memory before infection. 
Such weapon was intended to be used at rebellious colonies across the Solar system to "extinguish" rebels and making them 
controllable again.

Other group of scientists (called Beta) were working on genome modifications to raise more skillful, powerful and 
stronger soldiers for Outer Planets Army. Key concepts were to create few very specific types of soldiers:

1) Fast melee soldiers for neutralization squads, that can eliminate targets on spaceships without damaging the ship.
2) Strong melee soldiers with steel-like skin as bodyguards for VIPs for special events when armored soldiers cannot
participate.

The bunch of other potential experiments was planned for future, since the fewer people working on the ship the less 
chance of information leakage.

Genetic modifications don't always have good results and some species has to be destroyed. One of the scientists 
secretly started to collect and raise defective species in her lab that was located in one of old maintenance tunnels.
Her motivation was to try to save species with useful (as she thought) properties and then propose her results to 
higher command. One of such species was a replicator, it had an ability to revive dead human bodies (not rotten, but
with dead brain) making them to mutate in the new species.

As a result, over a few years there was collected a bunch of very dangerous species all of which were kept in 
cryostasis to prevent containment breach.

A series of tragic events has started from one of the scientists from the first group, he accidentally poked his finger
with a syringe filled with the virus. As the virus wasn't considered harmful or deadly, he didn't warn his colleagues
and continued his work as usual thinking that the dosage was low and no serious consequences will appear. However, the
virus started replicating in his body and a week later he woke up without being able to remember who he is and what's he
been doing on the ship. He was immediately transferred to the medical deck for diagnostics and nothing specific was found,
only high fatigue and apathy. He was left in the medical deck for rehabilitation for a week.

At this moment, patient zero has infected half of the medical deck employees and lots of his colleagues, and they've 
spread the virus across the entire ship. People started losing their memory one after another, at the moment when 
personnel of the ship realized that the virus has escaped it was too late, everybody on the ship has lost their memory.

After the incident, people started gather in groups trying to remember what is going on and who are they. One of the groups
has discovered a secret laboratory with dangerous creatures "sleeping" in cryostasis, any attempts to read scientist's 
records has failed because no one knew the decryption key, even the scientist herself. They've decided to thaw the species
thinking that they're men...

The slaughter was very fast, within a hour 99% of people were killed and their dead bodies were infected by the replicator.
The ship has become the most deadly place in the Solar system. 

The cycle has over, but the ship remained radio-silent and the high command from OPG decided to send a task force to 
investigate the reasons. As a preventive measure, an extermination squad was sent after the task force to "clean" the
ship if something gone wrong. The extermination squad wasn't informed what kind of ship they need to "clean".

Some people on the ship were still alive and able to build barricades to isolate themselves, most of them were located
in the food storage. However, one small group was able to turn the railguns on to prevent spreading the decease by 
some ship that still could accidentally spot the ship.

The transport ship with the task force and extermination squad has reaching the research ship, but from lit side of the 
satellite to not draw attention to the research ship. They're started slowly reach the ship and almost the landing stage
they were hit by one of the railguns, that was manual shot because automatic mode ignored the ship because its signature
was known. The transport ship had crashed on the loading bay, there was only one survivor from 20 people.

## Level design

High-level structure of the spaceship looks like this:

![Spaceship Structure](station_scheme.svg)

1) Loading bay - is the entry point to the spaceship, its purpose is to hold smaller transport ships and provide access
to the transport railway.
2) Research deck - top-security level area for classified experiments, the origin of the infection. This is the 
largest deck in the entire ship, it consists of few sections.
3) Medical deck - takes care about ill ship employees.
4) Bridge - is a control deck of the ship.
5) Sewage treatment plant - recycles various waste and supplies the station with clean water.
6) Hydroponics and food storage - produces vegetables and stores other food.

Every part of the spaceship is isolated from each other and communication is performed using the common railway that
operates in zero gravity and has no air for higher security. Every part of the ship has its own kitchen, bathrooms, 
and living rooms. This is needed for higher security to prevent info leakage, only high command and senior security
officers can go between decks freely.

Due to engine limitations, each deck should be a separate level.

Each level must be designed as a real working space, no abstract corridors should be used! Ideally each level should
have a high-level blueprint. Each level should have sci-fi aesthetics, but don't forget about "real usage" of the
environment.

Levels must be designed as a tightly packed set of rooms and corridors of various shapes. The action takes place on 
a spaceship where each square meter must be used wisely. There must be no long corridors leading to nowhere, long 
corridors only allowed if there is no place for rooms at the sides of the corridors.

### Loading bay

Loading bay is the entry hub to the spaceship, it has direct access to internal railway which allows employees of the
spaceship to transfer various loads across the ship. It has one loading area which can fit two transport ships at the
same time. It has a security area to check incoming cargo, incoming cargo could be any type starting from food and 
ending chemical reagents and various tools. The deck should have the kitchen, living block and bathrooms for personnel.

This is the starting point of the game, player appears at the beginning of the security area, the doors to crashed ship
are closed but player can see the fire. The first problem on his way is the automatic turrets that shoots unauthorized
persons on sight. The player must find a security computer to add himself to the list of authorized persons. After that
he can explore the area without any troubles. 

The deck has the following list of employees:

- 12 (4 per each shift) guards for the guard post. 2 works at their posts, 2 checks cargo documentation.
- 9 deck guards (3 per each shift), they're located in deck security post.
- 3 janitors
- 1 electrical technician
- 2 railway station technicians
- 1 plumber
- 2 cooks
- 2 loaders to load cargo on the train

32 people in total. This must be taken into account when designing the level. Everybody must have their sleeping place.

TODO: Write dialogs between these employees and their personal notes on their pocket PCs.

### Research deck

(INSERT IMAGE)

**(WIP)**

### Medical deck

(INSERT IMAGE)

**(WIP)**

### Bridge

(INSERT IMAGE)

**(WIP)**

### Sewage treatment plant

(INSERT IMAGE)

**(WIP)**

### Hydroponics and food storage

(INSERT IMAGE)

**(WIP)**

## Gameplay

### Main character description 

He's the extermination squad agent, that have arrived with his squad to kill everybody who working on the station and
remove all the files that may leak to outer world. 

Since the main character has military training and works in very hard conditions, it has a hazmat suit that covers
entire body and provides air filtering and decent armor level. Exoskeleton is used to provide additional strength
and to be able to lift heavy things to clean path. 

The character can walk, run, jump, use weapons and grenades.

### Camera

Typical 3rd person camera should be used, when shooting the camera should come closer to the shoulder providing a
better view. The camera should also avoid obstacles and do not let to see through walls. The camera can be rotated
freely around Y axis and have limited `[-90;90]` degrees range rotation around X axis.

### Inventory

Inventory is used to store all useful items that player can find. The inventory should allow player to use, examine,
and drop items. The capacity of the inventory is unlimited.

List of items:

- Small health pack
- Medium health pack
- Grenade
- Ammo
- M4 gun
- Rail gun
- Plasma gun
- ???

### Enemies

The amount of types of enemies is kinda low because the budget of the game is low too, there should be few kinds of 
enemies.

#### Standard "zombies"

A weak enemy which is basically slightly mutated version of typical employee, some of them can use weapons.
**(WIP)**

#### Fast zombies

A fast and dangerous melee enemy.
**(WIP)**

#### Heavy monsters

Slow, tough and very deadly monster, few hits of his arms is enough to kill main character.
**(WIP)**

#### Turrets

A surface-mounted automatic security turret that shoots everybody in range. It can be mounted on pretty much any surface,
even on ceilings and walls. Turrets can be re-configured using security computers, turrets can be in one of the following
modes:

- Off - completely disabled, usually this mode is very rarely used and mostly for maintenance.
- Hostile to everyone - special mode for containment breach situations.
- Hostile to non-authorized persons - basically it has a list of persons that has right clearance

**(WIP)**

## Storytelling

Since production resources are limited, various notes can be used for storytelling. There will be no cutscenes. As
a potential extension to notes, intercom can also be used to communicate with other survivors on the ship. However,
no other NPCs should be visible, it takes a lot of effort to have decent animations for NPCs. 