use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;

fn clear_screen() {
    print!("{}[2J{}[1;1H", 27 as char, 27 as char);
}

fn print_ui(hp: i32, gold: i32, lvl: i32, dmg: i32) {
    println!("╔══════════════════════════╗");
    println!("║       ⚔️ DUNGEON ⚔️      ║");
    println!("╠══════════════════════════╣");
    println!("║ HP:    {:<17} ║", format!("{}/100", hp.max(0)));
    println!("║ GOLD:  {:<17} ║", gold);
    println!("║ LVL:   {:<17} ║", lvl);
    println!("║ DMG:   {:<17} ║", dmg);
    println!("╚══════════════════════════╝");
}

fn main() {
    let mut hp: i32 = 100;
    let mut gold: i32 = 50;
    let mut lvl: i32 = 1;
    let mut dmg: i32 = 10;

    // Enable ANSI escape codes for Windows console (to clear screen properly)
    #[cfg(windows)]
    let _ = std::process::Command::new("cmd").args(["/c", "cls"]).status();

    println!("Welcome to the Dungeon...");

    println!("Press ENTER to begin...");

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    let mut turn = 1;

    loop {
        clear_screen();
        print_ui(hp, gold, lvl, dmg);

        if hp <= 0 {
            println!("\n💀 YOU DIED! The dungeon claims another soul.");
            println!("Restart the game to try again. Press ENTER to exit.");
            let mut end = String::new();
            io::stdin().read_line(&mut end).unwrap();
            break;
        }

        println!("\n[Turn {}] What will you do?", turn);
        println!("1. 🗡️    Explore the next room");
        println!("2. 🧪  Drink Health Potion (Costs 50 Gold, +20 HP)");
        println!("3. 🏃   Run Away (Exit Game)");
        print!("> ");
        io::stdout().flush().unwrap();

        input.clear();
        io::stdin().read_line(&mut input).unwrap();

        match input.trim() {
            "1" => {
                // Random-ish event based on turn
                let enemy_type = turn % 6;
                match enemy_type {
                    0 => {
                        let monster_dmg = 45 + (lvl * 12);
                        println!("\n🐲 A ferocious Dragon attacks!");
                        sleep(Duration::from_millis(500));
                        println!("You strike it for {} damage, but it breathes fire!", dmg);
                        hp -= monster_dmg;
                        println!("💥 -{} HP!", monster_dmg);
                        if hp > 0 { let reward = 50 + (lvl * 15); gold += reward; println!("✨ Looted {} GOLD!", reward); }
                    }
                    1 => {
                        let monster_dmg = 15 + (lvl * 5);
                        println!("\n👺 A sneaky Goblin ambushes you!");
                        hp -= monster_dmg;
                        println!("💥 -{} HP!", monster_dmg);
                        if hp > 0 { let reward = 15 + (lvl * 5); gold += reward; println!("✨ Looted {} GOLD!", reward); }
                    }
                    2 => {
                        let monster_dmg = 25 + (lvl * 8);
                        println!("\n💀 An Undead Skeleton rises from the ground!");
                        hp -= monster_dmg;
                        println!("💥 -{} HP!", monster_dmg);
                        if hp > 0 { let reward = 20 + (lvl * 5); gold += reward; println!("✨ Looted {} GOLD!", reward); }
                    }
                    3 => {
                        let monster_dmg = 10 + (lvl * 3);
                        println!("\n🦠 A Giant Slime engulfs you!");
                        hp -= monster_dmg;
                        println!("💥 -{} HP!", monster_dmg);
                        if hp > 0 { let reward = 5 + (lvl * 2); gold += reward; println!("✨ Looted {} GOLD!", reward); }
                    }
                    4 => {
                        let monster_dmg = 30 + (lvl * 10);
                        println!("\n🐺 A Dire Wolf pounces!");
                        hp -= monster_dmg;
                        println!("💥 -{} HP!", monster_dmg);
                        if hp > 0 { let reward = 25 + (lvl * 6); gold += reward; println!("✨ Looted {} GOLD!", reward); }
                    }
                    _ => {
                        let find = 10 + (lvl * 2);
                        gold += find;
                        println!("\n🦇 You killed some bats in the dark.");
                        println!("💰 Found {} GOLD!", find);
                    }
                }

                // Level up
                if turn % 5 == 0 {
                    lvl += 1;
                    dmg += 5;
                    println!("🌟 LEVEL UP! You are now Level {}! DMG increased to {}.", lvl, dmg);
                }
                turn += 1;
            }
            "2" => {
                if gold >= 50 {
                    gold -= 50;
                    hp = (hp + 20).min(100);
                    println!("\n🧪 Glug glug... Restored 20 HP! (-50 Gold)");
                } else {
                    println!("\n❌ Not enough gold! You need 50 Gold for a potion.");
                }
            }
            "3" => {
                println!("\nCoward. The dungeon awaits your return.");
                break;
            }
            _ => {
                println!("\nInvalid choice. The darkness waits...");
            }
        }

        println!("(Press ENTER to continue)");
        input.clear();
        io::stdin().read_line(&mut input).unwrap();
    }
}
