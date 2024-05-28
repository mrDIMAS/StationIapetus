# Station Iapetus (working title, subject to change)

3rd person shooter with tower defense mechanics.

## Level design

Relatively small level with a few spawn points for enemies, few destination points for them. 

## Gameplay

Waves of enemies spawn on the map and moving towards destination points, the player must kill most of them (at least 80%)
to pass the level.

### Camera

Typical 3rd person camera should be used, when shooting the camera should come closer to the shoulder providing a
better view. The camera should also avoid obstacles and do not let to see through walls. The camera can be rotated
freely around Y axis and have limited `[-90;90]` degrees range rotation around X axis.

**Status:** Done

### Inventory

Inventory is used to store all useful items that player can find. The inventory should allow player to use, examine,
and drop items. The capacity of the inventory is unlimited.

**Status:** Prototype is done

### Enemies

The amount of types of enemies is kinda low because the budget of the game is low too, there should be few kinds of 
enemies. Each type of enemy must have specific behaviour tree to make the game more interesting. Currently there is
only one behavior tree for every type of enemy.

#### Standard "zombies"

A weak enemy which is basically slightly mutated version of a typical employee, some of them can use weapons.
**(WIP)**

**Status:** Partially done

#### Fast zombies

A fast and dangerous melee enemy.
**(WIP)**

**Status:** Partially done

#### Heavy monsters

Slow, tough and very deadly monster, few hits of his arms is enough to kill main character.
**(WIP)**

**Status:** Partially done

#### Turrets

A surface-mounted automatic security turret that shoots everybody in range. It can be mounted on pretty much any surface,
even on ceilings and walls. Turrets can be re-configured using security computers, turrets can be in one of the following
modes:

- Off - completely disabled, usually this mode is very rarely used and mostly for maintenance.
- Hostile to everyone - special mode for containment breach situations.
- Hostile to non-authorized persons - basically it has a list of persons that has right clearance

**(WIP)**

**Status:** Partially done

### Interactive objects

#### Doors

Doors are used to provide access to specific areas on the station. Doors have clearance levels:

- D - Free access
- C - Restricted access (a.k.a. personnel only)
- B - High security
- A - High command access only

Player can use security terminals to change their clearance level. Clearance levels are not compatible between
decks, this means that if a player acquired level-3 clearance on Loading Bay deck, he won't be able to use
it on Medical deck for example.

Doors must be opened manually, it means that when potential user comes close enough to a door, it should show clearance
level and ask user if it should be opened. 

Some doors can be opened only if user has appropriate key card.

**Status:** Partially done

#### Elevators

Floors of vertical maps can be connected via elevators. Elevators should support any amount of levels. Each floor should
have a button to call the elevator.

**Status:** Not ready

#### Items

- Small health pack - restores 20% of health.
- Medium health pack - restores 40% of health.
- Grenade - consumable item, it can be thrown by player.
- Ammo - universal ammo that is basically an energy cell.
- Glock - semi-automatic pistol designed to be used with energy ammo 
- M4 gun - classic rifle designed to be used with energy ammo
- Rail gun - powerful gun with high-energy projectiles that are able to penetrate multiple targets.
- Plasma gun - powerful energy gun that shoots plasma balls.
- Key-card - customizable key card.

**Status:** Partially done