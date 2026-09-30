package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Generated;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A union no declaration names is an enum to a Rust host, a variant for each member: a declared one
 * holding its handle, a primitive Rust's own value. A behavior answering one is told which case the
 * value is, and a case of a member sum comes back as that member; a host implementation answering
 * one hands the library the variant it chose, a primitive carried into the union.
 */
class ARustHostHandsOverAndIsHandedAUnionTest {

    private static final String SHOP = """
            module shop exposing ( Money, Free, Paid, Owed, Settled, charge, chargedFree,
                                   quantityOf, flaggedOf, labelOf, doubledQuantity, rated )

            data Money = Int
                invariant notNegative = value >= 0

            data Free
            data Paid = { amount: Money }
            data Waived = { reason: String }
            data Owed = { amount: Money, overdue: Bool }
            data Settled = Free | Paid | Waived

            behavior charge : (paid: Int) -> Owed | Settled
            let charge (paid) =
                if paid > 1 then Waived { reason = "goodwill" }
                else if paid > 0 then Free
                else Owed { amount = Money(1), overdue = true }

            behavior chooseCharge : (paid: Int) -> Owed | Settled

            behavior chargedFree : (paid: Int) -> Bool
                depends on chooseCharge
            let chargedFree (paid, chooseCharge) = match chooseCharge(paid) with
                | Free -> true
                | Paid -> false
                | Waived -> false
                | Owed -> false

            behavior quantityOf : (paid: Int) -> Int | Free
            let quantityOf (paid) = if paid > 0 then paid else Free

            behavior flaggedOf : (paid: Int) -> Bool | Free
            let flaggedOf (paid) = if paid > 0 then paid > 1 else Free

            behavior labelOf : (paid: Int) -> String | Free
            let labelOf (paid) = if paid > 0 then "paid" else Free

            behavior chooseQuantity : (paid: Int) -> Int | Free

            behavior doubledQuantity : (paid: Int) -> Int
                depends on chooseQuantity
            let doubledQuantity (paid, chooseQuantity) = match chooseQuantity(paid) with
                | Int as n -> n * 2
                | Free -> -1

            behavior rated : (amount: Decimal) -> Decimal | Free
            let rated (amount) = if amount == 0m then Free else amount
            """;

    private static final String HOST = """
            use unions::shop::{
                self, ChargedFree, ChooseCharge, ChooseChargeImplementation, ChooseQuantity,
                ChooseQuantityImplementation, DecimalOrFree, DoubledQuantity, Free, FreeOrInt, Money, Owed,
                OwedOrSettled, SettledCase,
            };
            use unions::{Decimal, HostError, Library, Run};

            struct FreeFromTwo;

            impl ChooseCharge for FreeFromTwo {
                fn apply<'run>(&self, run: &mut Run<'run>, paid: i64) -> Result<OwedOrSettled<'run>, HostError> {
                    Ok(if paid >= 2 {
                        OwedOrSettled::Settled(Free::new(run)?.into_result().map_err(|it| it.code().to_owned())?.into())
                    } else {
                        let amount = Money::new(run, 1)?.into_result().map_err(|it| it.code().to_owned())?;
                        OwedOrSettled::Owed(Owed::new(run, amount, true)?.into_result().map_err(|it| it.code().to_owned())?)
                    })
                }
            }

            struct Halved;

            impl ChooseQuantity for Halved {
                fn apply<'run>(&self, _run: &mut Run<'run>, paid: i64) -> Result<FreeOrInt<'run>, HostError> {
                    Ok(if paid > 0 { FreeOrInt::Int(paid / 2) } else { FreeOrInt::Free(free()?) })
                }
            }

            fn free<'run>() -> Result<Free<'run>, HostError> {
                Err("no run to make one in".into())
            }

            fn said(charge: OwedOrSettled<'_>) -> String {
                match charge {
                    OwedOrSettled::Owed(owed) => format!("owed {}", owed.amount().value()),
                    OwedOrSettled::Settled(settled) => match settled.case() {
                        SettledCase::Free(_) => "free".to_owned(),
                        SettledCase::Paid(paid) => format!("paid {}", paid.amount().value()),
                        SettledCase::Kept(_) => "a case the model keeps".to_owned(),
                    },
                }
            }

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                let chooser = ChooseChargeImplementation::new(&library, FreeFromTwo);
                let halved = ChooseQuantityImplementation::new(&library, Halved);
                library
                    .run(|run| {
                        for paid in [0, 1, 2] {
                            println!("charge {paid}: {}", said(shop::charge(run, paid).unwrap()));
                        }
                        for paid in [0, 3] {
                            let quantity = match shop::quantityOf(run, paid).unwrap() {
                                FreeOrInt::Int(n) => format!("{n}"),
                                FreeOrInt::Free(_) => "free".to_owned(),
                            };
                            let flagged = match shop::flaggedOf(run, paid).unwrap() {
                                unions::shop::BoolOrFree::Bool(b) => format!("{b}"),
                                unions::shop::BoolOrFree::Free(_) => "free".to_owned(),
                            };
                            let label = match shop::labelOf(run, paid).unwrap() {
                                unions::shop::FreeOrString::String(s) => s,
                                unions::shop::FreeOrString::Free(_) => "free".to_owned(),
                            };
                            println!("{paid}: {quantity} {flagged} {label}");
                        }
                        for amount in [Decimal::of(0, 0), Decimal::of(125, 2)] {
                            match shop::rated(run, &amount).unwrap() {
                                DecimalOrFree::Decimal(d) => println!("rated: {}e-{}", d.unscaled(), d.scale()),
                                DecimalOrFree::Free(_) => println!("rated: free"),
                            }
                        }
                        let charged = ChargedFree::bind(&library, &chooser);
                        println!("charged free: {} {}", charged.call(run, 1).unwrap(), charged.call(run, 2).unwrap());
                        let doubled = DoubledQuantity::bind(&library, &halved);
                        println!("doubled: {}", doubled.call(run, 9).unwrap());
                        println!("doubled free: {}", doubled.call(run, 0).unwrap_err());
                    })
                    .unwrap();
            }
            """;

    @Test
    void aUnionIsAnEnumOfItsMembers(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHOP)), into.resolve("native"));
        Generated binding = RustHost.generated(library, into.resolve("binding"),
                "unions");

        String said = RustHost.ran(into, binding, "unions", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                charge 0: owed 1
                charge 1: free
                charge 2: a case the model keeps
                0: free free free
                3: 3 true paid
                rated: free
                rated: 125e-2
                charged free: false true
                doubled: 8
                doubled free: a host implementation failed: no run to make one in
                """);
    }
}
