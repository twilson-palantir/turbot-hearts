#![allow(dead_code)]

#[macro_use]
extern crate bitflags;

extern crate rand;

use rand::{thread_rng, Rng};
use std::fmt::{self, Write};

const RANKS: [char; 13] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'T', 'J', 'Q', 'K', 'A'
];
const SUITS: [char; 4] = ['C', 'D', 'H', 'S'];

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
struct Card(u8);

impl Card {
    const TWO_CLUBS: Card = Card(0);

    fn suit(self) -> Cards {
        Cards::from_bits(0x1fff << (16 * (self.0 / 16))).unwrap()
    }

    fn as_cards(self) -> Cards {
        Cards::from_bits(1 << (self.0)).unwrap()
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char(RANKS[(self.0 % 13) as usize])?;
        f.write_char(SUITS[(self.0 / 13) as usize])?;
        Ok(())
    }
}

bitflags! {
    struct Cards: u64 {
        const CHARGEABLE = 0x0400_1000_0200_0100;
        const NINES = 0x0080_0080_0080_0080;
        const CLUBS = 0x0000_0000_0000_1fff;
        const DIAMONDS = 0x0000_0000_1fff_0000;
        const HEARTS = 0x0000_1fff_0000_0000;
        const SPADES = 0x1fff_0000_0000_0000;
        const TWO_CLUBS = 0x0000_0000_0000_0001;
        const TEN_CLUBS = Self::CLUBS.bits & Self::CHARGEABLE.bits;
        const JACK_DIAMONDS = Self::DIAMONDS.bits & Self::CHARGEABLE.bits;
        const ACE_HEARTS = Self::HEARTS.bits & Self::CHARGEABLE.bits;
        const QUEEN_SPADES = Self::SPADES.bits & Self::CHARGEABLE.bits;
        const POINTS = Self::HEARTS.bits | Self::QUEEN_SPADES.bits | Self::JACK_DIAMONDS.bits;
    }
}

impl Cards {
    fn len(self) -> u32 {
        self.bits.count_ones()
    }

    fn max(self) -> Card {
        Card(63 - self.bits.leading_zeros() as u8)
    }

    fn suit(self) -> Self {
        for suit in &[Cards::SPADES, Cards::HEARTS, Cards::DIAMONDS, Cards::CLUBS] {
            if self.intersects(*suit) {
                return *suit;
            }
        }
        Cards::empty()
    }

    fn parse(s: &str) -> Self {
        let mut bits = 0;
        for card in s.split(' ') {
            let mut chars = card.chars();
            let suit = chars.next_back().unwrap();
            let suit = SUITS.iter().position(|&s| s == suit).unwrap();
            for rank in chars {
                let rank = RANKS.iter().position(|&r| r == rank).unwrap();
                bits |= 1 << (16 * suit + rank);
            }
        }
        Cards::from_bits(bits).unwrap()
    }
}

impl fmt::Display for Cards {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bits = self.bits;
        let mut first = true;
        for i in (0..4).rev() {
            let mut suit = (bits >> (16 * i)) & 0xffff;
            if suit != 0 {
                if !first {
                    f.write_char(' ')?;
                }
                while suit != 0 {
                    let next = 63 - suit.leading_zeros();
                    f.write_char(RANKS[next as usize])?;
                    suit -= 1 << next;
                }
                f.write_char(SUITS[i as usize])?;
                first = false;
            }
        }
        Ok(())
    }
}

impl std::ops::BitOr<Card> for Cards {
    type Output = Self;

    fn bitor(self, rhs: Card) -> Self {
        self | rhs.as_cards()
    }
}

impl std::ops::BitOrAssign<Card> for Cards {
    fn bitor_assign(&mut self, rhs: Card) {
        self.bitor_assign(rhs.as_cards());
    }
}

impl std::ops::Sub<Card> for Cards {
    type Output = Self;

    fn sub(self, rhs: Card) -> Self {
        self - rhs.as_cards()
    }
}

impl std::ops::SubAssign<Card> for Cards {
    fn sub_assign(&mut self, rhs: Card) {
        self.sub_assign(rhs.as_cards());
    }
}

struct FullState {
    /// The cards currently in each player's hand
    hand: [Cards; 4],
    /// The cards each player has won
    won: [Cards; 4],
    /// The cards that were charged
    charged: Cards,
    /// The suits that have been led
    led_suits: Cards,
    /// The card that led the current trick, or empty
    trick_lead: Cards,
    /// The cards in the current trick
    trick: Cards,
}

fn deal_hands() -> [Cards; 4] {
    let mut deck = [Card(0); 52];
    for i in 0..52 {
        deck[i] = Card((16 * (i / 13) + (i % 13)) as u8);
    }
    thread_rng().shuffle(&mut deck);
    let mut hands = [Cards::empty(); 4];
    for i in 0..52 {
        hands[i / 13] |= deck[i];
    }
    hands
}

fn trick_winner(trick: Cards, lead: Card) -> Card {
    (trick & lead.suit()).max()
}

fn is_nined(trick: Cards, lead: Card) -> bool {
    !(Cards::NINES & trick & lead.suit()).is_empty()
}

fn holder_of(hand: [Cards; 4], card: Card) -> usize {
    let card = card.as_cards();
    match (
        hand[0].intersects(card),
        hand[1].intersects(card),
        hand[2].intersects(card),
        hand[3].intersects(card),
    ) {
        (true, _, _, _) => 0,
        (_, true, _, _) => 1,
        (_, _, true, _) => 2,
        (_, _, _, _) => 3,
    }
}

fn opt_hand(hand: [Cards; 4]) -> [Cards; 4] {
    let player = holder_of(hand, Card::TWO_CLUBS);
    let mut opt_charged = Cards::empty();
    let mut opt_won = opt_post_charge(player, hand, opt_charged);
    for i in 0..4 {
        let mut opt_money = money(opt_won, opt_charged, i);
        let mut chargeable = hand[i] & Cards::CHARGEABLE;
        while chargeable != Cards::empty() {
            let card = chargeable.max();
            let next_charged = opt_charged | card;
            let next_won = opt_post_charge(player, hand, next_charged);
            let next_money = money(next_won, next_charged, i);
            if next_money > opt_money {
                opt_charged = next_charged;
                opt_won = next_won;
                opt_money = next_money;
            }
        }
    }
    opt_won
}

fn opt_post_charge(player: usize, hand: [Cards; 4], charged: Cards) -> [Cards; 4] {
    opt_inner(
        player,
        hand,
        [
            Cards::empty(),
            Cards::empty(),
            Cards::empty(),
            Cards::empty(),
        ],
        charged,
        Cards::empty(),
        None,
        Cards::empty(),
    )
}

fn opt_inner(
    player: usize,
    hand: [Cards; 4],
    won: [Cards; 4],
    charged: Cards,
    led_suits: Cards,
    lead: Option<Card>,
    trick: Cards,
) -> [Cards; 4] {
    let played = won[0] | won[1] | won[2] | won[3];
    if played == Cards::all() {
        return won;
    }
    let hearts_broken = played.intersects(Cards::HEARTS);
    let plays = legal_plays(
        hand[player] - trick,
        charged,
        led_suits,
        lead,
        hearts_broken,
    );
    let lost = if trick == Cards::empty() {
        Cards::empty()
    } else {
        trick - trick_winner(trick, lead.unwrap()).as_cards()
    };
    let mut plays = distinct_plays(plays, played | lost, charged);

    let trick_size = trick.len();
    let mut opt_money = -1000;
    let mut opt_won = [
        Cards::empty(),
        Cards::empty(),
        Cards::empty(),
        Cards::empty(),
    ];
    while plays != Cards::empty() {
        let play = plays.max();
        plays -= play;

        let finishes_trick = trick_size == 7
            || (trick_size == 3 && (played.len() == 48 || !is_nined(trick | play, lead.unwrap())));

        let next_lead = if trick_size == 0 {
            Some(play)
        } else if finishes_trick {
            None
        } else {
            lead
        };

        let next_trick = if finishes_trick {
            Cards::empty()
        } else {
            trick | play
        };

        let next_hand = if finishes_trick {
            [
                hand[0] - (trick | play),
                hand[1] - (trick | play),
                hand[2] - (trick | play),
                hand[3] - (trick | play),
            ]
        } else {
            hand
        };

        let next_player = if finishes_trick {
            let winning_card = trick_winner(trick | play, lead.unwrap());
            holder_of(hand, winning_card)
        } else {
            (player + 1) % 4
        };

        let next_led_suits = if finishes_trick {
            led_suits | lead.unwrap().suit()
        } else {
            led_suits
        };

        let mut next_won = won;
        if finishes_trick {
            next_won[next_player] |= trick | play;
        }

        let resulting_won = opt_inner(
            next_player,
            next_hand,
            next_won,
            charged,
            next_led_suits,
            next_lead,
            next_trick,
        );
        let resulting_money = money(resulting_won, charged, player);
        if resulting_money > opt_money {
            opt_money = resulting_money;
            opt_won = resulting_won;
        }
    }
    opt_won
}

fn legal_plays(
    hand: Cards,
    charged: Cards,
    led_suits: Cards,
    lead: Option<Card>,
    hearts_broken: bool,
) -> Cards {
    let mut plays = hand;
    let suit = lead.map(Card::suit).unwrap_or(Cards::empty());

    // If this is the first trick
    if led_suits.is_empty() {
        // and you have the two of clubs
        if plays.intersects(Cards::TWO_CLUBS) {
            // you must play it
            return Cards::TWO_CLUBS;
        }

        // If you have a non-point card
        if plays.intersects(!Cards::POINTS) {
            // you cannot play a point
            plays -= Cards::POINTS;

        // otherwise, if you have the jack of diamonds
        } else if plays.intersects(Cards::JACK_DIAMONDS) {
            // you must play it
            return Cards::JACK_DIAMONDS;

        // otherwise, if you have the queen of spades
        } else if plays.intersects(Cards::QUEEN_SPADES) {
            // you must play it
            return Cards::QUEEN_SPADES;
        }
    }

    // If you're leading the trick
    if suit.is_empty() {
        // and hearts are not broken, and you have a non-heart
        if !hearts_broken && plays.intersects(!Cards::HEARTS) {
            // you may not lead hearts
            plays -= Cards::HEARTS;
        }

        // If you have a non-charged playable card
        if plays.intersects(!(Cards::CHARGEABLE & charged)) {
            // for each chargeable card
            for card in &[
                Cards::QUEEN_SPADES,
                Cards::ACE_HEARTS,
                Cards::TEN_CLUBS,
                Cards::JACK_DIAMONDS,
            ] {
                // if that card is charged, and its suit has not been led
                if charged.intersects(*card) && !led_suits.intersects(*card) {
                    // you may not lead it
                    plays -= *card;
                }
            }
        }

    // otherwise
    } else {
        // if you have a card in suit
        if plays.intersects(suit) {
            // you must play a card in suit
            plays &= suit;

            // and if this is the first lead of the suit and you have multiple plays
            if !led_suits.intersects(suit) && plays.len() > 1 {
                // you may not play the charged card in suit
                plays -= charged & suit;
            }
        }
    }

    plays
}

/// Remove from plays all cards that have the same trick taking and scoring
/// potential as some larger card in plays.
///
/// Generally, two cards will be equivalent if they are the same suit and all
/// cards between them have already been played, or are in hand.
///
/// However, special cards (nines, QS, JD, TC, and AH if charged) are not
/// equivalent to any other card AND if they have not been played, no cards
/// that span them are equivalent either.
fn distinct_plays(plays: Cards, played: Cards, charged: Cards) -> Cards {
    let special = Cards::NINES | Cards::QUEEN_SPADES | Cards::JACK_DIAMONDS | Cards::TEN_CLUBS
        | (charged & Cards::ACE_HEARTS);
    let special_plays = plays & special;
    let mut magic = (plays - special_plays).bits;
    let equivalent_blocks = magic | played.bits;
    for _ in 0..11 {
        magic = (magic | (magic >> 1)) & equivalent_blocks;
    }
    magic += ((!magic) << 1) | 0x0001_0001_0001_0001;
    special_plays | Cards::from_bits(plays.bits & (magic >> 1)).unwrap()
}

fn money(won: [Cards; 4], charged: Cards, player: usize) -> i32 {
    let me = score(won[player], charged);
    let left = score(won[(player + 1) % 4], charged);
    let across = score(won[(player + 2) % 4], charged);
    let right = score(won[(player + 3) % 4], charged);
    left + across + right - 3 * me
}

fn score(won: Cards, charged: Cards) -> i32 {
    let hearts = match (
        (won & Cards::HEARTS).len() as i32,
        charged.intersects(Cards::ACE_HEARTS),
    ) {
        (cnt, true) => 2 * cnt,
        (cnt, _) => cnt,
    };
    let queen = match (
        won.intersects(Cards::QUEEN_SPADES),
        charged.intersects(Cards::QUEEN_SPADES),
    ) {
        (true, true) => 26,
        (true, false) => 13,
        _ => 0,
    };
    let jack = match (
        won.intersects(Cards::JACK_DIAMONDS),
        charged.intersects(Cards::JACK_DIAMONDS),
    ) {
        (true, true) => -20,
        (true, false) => -10,
        _ => 0,
    };
    let ten = match (
        won.intersects(Cards::TEN_CLUBS),
        charged.intersects(Cards::TEN_CLUBS),
    ) {
        (true, true) => 4,
        (true, false) => 2,
        _ => 1,
    };
    if won.intersects(Cards::QUEEN_SPADES) && won.contains(Cards::HEARTS) {
        ten * (jack - hearts - queen)
    } else {
        ten * (jack + hearts + queen)
    }
}

fn main() {
    //let c1 = Cards::parse("AJT5S J63H 96D A953C");
    //let c2 = Cards::parse("9732S T92H K7D KT74C");
    //let c3 = Cards::parse("KQ6S A5H JT542D Q82C");
    //let c4 = Cards::parse("84S KQ874H AQ83D J6C");
    //let opt = opt_hand([c1, c2, c3, c4]);
    //println!("{}, {}, {}, {}", opt[0], opt[1], opt[2], opt[3]);
    println!("{}", Card(49).suit());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cards_display() {
        assert_eq!(
            format!("{}", Cards::CHARGEABLE | Cards::NINES),
            "Q9S A9H J9D T9C"
        );
    }

    #[test]
    fn test_cards_parse() {
        assert_eq!(
            Cards::parse("Q9S A9H J9D T9C"),
            Cards::CHARGEABLE | Cards::NINES
        );
    }

    #[test]
    fn test_score() {
        let c = Cards::parse("AJT5S J63H 96D A953C");
        assert_eq!(score(c, Cards::empty()), 3);
        assert_eq!(score(c, Cards::QUEEN_SPADES | Cards::TEN_CLUBS), 3);
        assert_eq!(score(c, Cards::JACK_DIAMONDS), 3);
        assert_eq!(score(c, Cards::ACE_HEARTS), 6);
        assert_eq!(score(c, Cards::ACE_HEARTS | Cards::TEN_CLUBS), 6);
        let c = Cards::parse("973S T92H K7D KT74C");
        assert_eq!(score(c, Cards::empty()), 6);
        assert_eq!(score(c, Cards::QUEEN_SPADES | Cards::TEN_CLUBS), 12);
        assert_eq!(score(c, Cards::JACK_DIAMONDS), 6);
        assert_eq!(score(c, Cards::ACE_HEARTS), 12);
        assert_eq!(score(c, Cards::ACE_HEARTS | Cards::TEN_CLUBS), 24);
        let c = Cards::parse("KQ6S A5H JT542D Q82C");
        assert_eq!(score(c, Cards::empty()), 5);
        assert_eq!(score(c, Cards::QUEEN_SPADES | Cards::TEN_CLUBS), 18);
        assert_eq!(score(c, Cards::JACK_DIAMONDS), -5);
        assert_eq!(score(c, Cards::ACE_HEARTS), 7);
        assert_eq!(score(c, Cards::ACE_HEARTS | Cards::TEN_CLUBS), 7);
        let c = Cards::parse("84S KQ874H AQ83D J6C");
        assert_eq!(score(c, Cards::empty()), 5);
        assert_eq!(score(c, Cards::QUEEN_SPADES | Cards::TEN_CLUBS), 5);
        assert_eq!(score(c, Cards::JACK_DIAMONDS), 5);
        assert_eq!(score(c, Cards::ACE_HEARTS), 10);
        assert_eq!(score(c, Cards::ACE_HEARTS | Cards::TEN_CLUBS), 10);
        let c = Cards::HEARTS | Cards::QUEEN_SPADES;
        assert_eq!(score(c, Cards::empty()), -26);
        assert_eq!(score(c, Cards::QUEEN_SPADES | Cards::TEN_CLUBS), -39);
        assert_eq!(score(c, Cards::JACK_DIAMONDS), -26);
        assert_eq!(score(c, Cards::ACE_HEARTS), -39);
        assert_eq!(score(c, Cards::ACE_HEARTS | Cards::TEN_CLUBS), -39);
    }

    #[test]
    fn test_money() {
        let c1 = Cards::parse("AJT5S J63H 96D A953C");
        let c2 = Cards::parse("973S T92H K7D KT74C");
        let c3 = Cards::parse("KQ6S A5H JT542D Q82C");
        let c4 = Cards::parse("84S KQ874H AQ83D J6C");
        let won = [c1, c2, c3, c4];
        assert_eq!(money(won, Cards::empty(), 0), 7);
        assert_eq!(money(won, Cards::empty(), 1), -5);
        assert_eq!(money(won, Cards::empty(), 2), -1);
        assert_eq!(money(won, Cards::empty(), 3), -1);
        assert_eq!(money(won, Cards::ACE_HEARTS, 0), 11);
        assert_eq!(money(won, Cards::ACE_HEARTS, 1), -13);
        assert_eq!(money(won, Cards::ACE_HEARTS, 2), 7);
        assert_eq!(money(won, Cards::ACE_HEARTS, 3), -5);
        assert_eq!(money(won, Cards::TEN_CLUBS, 0), 13);
        assert_eq!(money(won, Cards::TEN_CLUBS, 1), -23);
        assert_eq!(money(won, Cards::TEN_CLUBS, 2), 5);
        assert_eq!(money(won, Cards::TEN_CLUBS, 3), 5);
    }

    #[test]
    fn test_distinct_plays() {
        assert_eq!(
            distinct_plays(
                Cards::parse("AQT8642S KJ9753H AQT8642D KJ9753C"),
                Cards::parse("KJ9753S AQT8642H KJ9753D AQT8642C"),
                Cards::empty()
            ),
            Cards::parse("AQTS K97H AD K97C")
        );
        assert_eq!(
            distinct_plays(Cards::parse("K7S"), Cards::parse("QJT98S"), Cards::empty()),
            Cards::parse("KS")
        );
        assert_eq!(
            distinct_plays(Cards::parse("Q7S"), Cards::parse("JT98S"), Cards::empty()),
            Cards::parse("Q7S")
        );
        println!(
            "{}",
            distinct_plays(Cards::parse("AKH"), Cards::empty(), Cards::empty())
        );
        assert_eq!(
            distinct_plays(Cards::parse("AKH"), Cards::empty(), Cards::empty()),
            Cards::parse("AH")
        );
        assert_eq!(
            distinct_plays(Cards::parse("AKH"), Cards::empty(), Cards::ACE_HEARTS),
            Cards::parse("AKH")
        );
        assert_eq!(
            distinct_plays(
                Cards::parse("A2D"),
                Cards::parse("KQJT9876543D"),
                Cards::empty()
            ),
            Cards::parse("AD")
        );
    }

    #[test]
    fn test_legal_plays_lead() {
        assert_eq!(
            legal_plays(
                Cards::parse("AQ54S 543H AKQ2C 83D"),
                Cards::empty(),
                Cards::empty(),
                Cards::empty(),
                false
            ),
            Cards::parse("2C")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AQ54S 543H AKC 83D"),
                Cards::empty(),
                Cards::CLUBS,
                Cards::empty(),
                false
            ),
            Cards::parse("AQ54S AKC 83D")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AT543H"),
                Cards::empty(),
                Cards::SPADES | Cards::DIAMONDS | Cards::CLUBS,
                Cards::empty(),
                false
            ),
            Cards::parse("AT543H")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AQ54S 543H AKC 83D"),
                Cards::QUEEN_SPADES,
                Cards::DIAMONDS | Cards::CLUBS,
                Cards::empty(),
                true
            ),
            Cards::parse("A54S 543H AKC 83D")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AQ54S 543H AKC 83D"),
                Cards::QUEEN_SPADES,
                Cards::SPADES | Cards::CLUBS,
                Cards::empty(),
                true
            ),
            Cards::parse("AQ54S 543H AKC 83D")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AKQJT9H JD"),
                Cards::JACK_DIAMONDS,
                Cards::SPADES | Cards::CLUBS,
                Cards::empty(),
                false
            ),
            Cards::parse("JD")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AKQJT9H JD"),
                Cards::JACK_DIAMONDS,
                Cards::SPADES | Cards::CLUBS | Cards::DIAMONDS,
                Cards::empty(),
                false
            ),
            Cards::parse("JD")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AKQJT9H JD"),
                Cards::JACK_DIAMONDS,
                Cards::SPADES | Cards::CLUBS,
                Cards::empty(),
                true
            ),
            Cards::parse("AKQJT9H")
        );
    }

    #[test]
    fn test_legal_plays_follow_first_trick() {
        assert_eq!(
            legal_plays(
                Cards::parse("AQ54S 543H AKQ3C 83D"),
                Cards::empty(),
                Cards::empty(),
                Cards::parse("2C"),
                false
            ),
            Cards::parse("AKQ3C")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AQ5432S 8543H T83D"),
                Cards::empty(),
                Cards::empty(),
                Cards::parse("2C"),
                false
            ),
            Cards::parse("A5432S T83D")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("QS AKQJT987654H JD"),
                Cards::empty(),
                Cards::empty(),
                Cards::parse("2C"),
                false
            ),
            Cards::parse("JD")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("QS AKQJT9876542H"),
                Cards::QUEEN_SPADES,
                Cards::empty(),
                Cards::parse("2C"),
                false
            ),
            Cards::parse("QS")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AKQJT98765432H"),
                Cards::QUEEN_SPADES,
                Cards::empty(),
                Cards::parse("2C"),
                false
            ),
            Cards::parse("AKQJT98765432H")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AKQJT98765432H"),
                Cards::ACE_HEARTS,
                Cards::empty(),
                Cards::parse("2C"),
                false
            ),
            Cards::parse("AKQJT98765432H")
        );
    }

    #[test]
    fn test_legal_plays_follow() {
        assert_eq!(
            legal_plays(
                Cards::parse("AQS 54H AQ3C 83D"),
                Cards::QUEEN_SPADES,
                Cards::CLUBS | Cards::DIAMONDS,
                Cards::parse("7C"),
                false
            ),
            Cards::parse("AQ3C")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AQS 54H 83D"),
                Cards::QUEEN_SPADES,
                Cards::CLUBS | Cards::DIAMONDS,
                Cards::parse("7C"),
                false
            ),
            Cards::parse("AQS 54H 83D")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("AQS 54H AQ3C 83D"),
                Cards::QUEEN_SPADES,
                Cards::CLUBS | Cards::DIAMONDS,
                Cards::parse("7S"),
                false
            ),
            Cards::parse("AS")
        );
        assert_eq!(
            legal_plays(
                Cards::parse("QS 54H AQ3C 83D"),
                Cards::QUEEN_SPADES,
                Cards::CLUBS | Cards::DIAMONDS,
                Cards::parse("7S"),
                false
            ),
            Cards::parse("QS")
        );
    }

    #[test]
    fn test_trick_winner() {
        assert_eq!(
            trick_winner(Cards::parse("A8S 96H"), Cards::parse("6H")),
            Cards::parse("9H")
        );
    }

    #[test]
    fn test_opt_hand() {
        let opt = opt_hand([Cards::SPADES, Cards::HEARTS, Cards::CLUBS, Cards::DIAMONDS]);
        println!("{}, {}, {}, {}", opt[0], opt[1], opt[2], opt[3]);
        assert_eq!(1, 2)
    }
}
