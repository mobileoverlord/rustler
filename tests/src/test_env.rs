use rustler::{NifEnv, NifTerm, NifResult, NifEncoder};
use rustler::env::{OwnedEnv, SavedTerm};
use rustler::types::atom;
use rustler::types::list::NifListIterator;
use rustler::types::pid::NifPid;
use std::thread;

// Send a message to several PIDs.
pub fn send_all<'a>(env: NifEnv<'a>, args: &[NifTerm<'a>]) -> NifResult<NifTerm<'a>> {
    let pids: Vec<NifPid> = args[0].decode()?;
    let message = args[1];

    for pid in pids {
        env.send(&pid, message);
    }

    Ok(message)
}

pub fn sublists<'a>(env: NifEnv<'a>, args: &[NifTerm<'a>]) -> NifResult<NifTerm<'a>> {
    // This is a "threaded NIF": it spawns a thread that sends a message back
    // to the calling thread later.
    let pid = env.pid();

    // Our worker thread will need an environment.  We can't ship `env` to the
    // other thread, because the Erlang VM is going to tear it down as soon as
    // we return from this NIF. So we use an `OwnedEnv`.
    let mut my_env = OwnedEnv::new();

    // Start by taking the argument (which should be a list), and copying it
    // into `my_env`, and reversing it. We can use `my_env.save()` to save
    // terms in a form that doesn't have a lifetime parameter.
    let saved_reversed_list = my_env.run(|env| -> NifResult<SavedTerm> {
        let list_arg = args[0].in_env(env);
        Ok(my_env.save(list_arg.list_reverse()?))
    })?;

    // Start the worker thread. This `move` closure takes ownership of both
    // `my_env` and `saved_reversed_list`.
    thread::spawn(move || {
        // Use `.send()` to get a `NifEnv` from our `OwnedEnv`,
        // run some rust code, and finally send the result back to `pid`.
        my_env.send_and_clear(&pid, |env| {
            let result: NifResult<NifTerm> = (|| {
                let reversed_list = saved_reversed_list.load(env);
                let iter: NifListIterator = try!(reversed_list.decode());

                let empty_list = Vec::<NifTerm>::new().encode(env);
                let mut all_sublists = vec![empty_list];

                for element in iter {
                    for i in 0 .. all_sublists.len() {
                        let new_list = all_sublists[i].list_prepend(element);
                        all_sublists.push(new_list);
                    }
                }

                Ok(all_sublists.encode(env))
            })();

            match result {
                Err(_err) => env.error_tuple("test failed".encode(env)),
                Ok(term) => term
            }
        });
    });

    Ok(atom::ok().to_term(env))
}
