use super::*;
fn state(size: i32, n: usize) -> State {
    let mut s = State::default();
    s.tick = 120; s.target = 123; s.blocked = 7; s.wind = [-1.5, 0.25];
    s.transformed = [1,2,3,4,5,6]; s.packed = [6,5,4,3,2,1];
    s.imported = [17;6]; s.withdrawn = [3;6];
    for i in 0..n as i32 {
        s.air.insert(((i-4096)*size, 64*size, -32*size, size), [u64::from(i as u32)+1;6]);
        s.deposits.insert(((i-4096)*size, 63*size, -32*size), [u64::from(i as u32)+7;6]);
    }
    s
}
fn encoded(s: &State) -> Vec<u8> { let mut b=Vec::new(); s.encode(&mut b); b }
fn send(sent: &mut Sent, s: &State) -> Vec<u8> { sent.write(&Arc::new(Image::capture(s))).unwrap() }
#[test]
fn ambient_full_delta_removals_and_empty_state_match_at_all_scales() {
    for size in [1,2,4] {
        let mut host=state(size,8); let mut sent=Sent::default();
        let mut peer=read(&send(&mut sent,&host),&State::default()).unwrap();
        assert_eq!(encoded(&host),encoded(&peer));
        let key=*host.air.keys().next().unwrap(); host.air.get_mut(&key).unwrap()[4]+=73;
        host.air.pop_last(); host.deposits.pop_first(); host.tick+=4; host.target+=4;
        let b=send(&mut sent,&host); assert_eq!(b[0],0);
        peer=read(&b,&peer).unwrap(); assert_eq!(encoded(&host),encoded(&peer));
        host.air.values_mut().for_each(|m|*m=[0;6]); host.deposits.clear();
        peer=read(&send(&mut sent,&host),&peer).unwrap();
        assert!(peer.air.is_empty()&&peer.deposits.is_empty()); assert_eq!(encoded(&host),encoded(&peer));
    }
}
#[test]
fn ambient_same_shared_sample_costs_no_payload_and_dense_turnover_uses_full() {
    let mut host=state(1,128); let image=Arc::new(Image::capture(&host)); let mut sent=Sent::default();
    assert!(sent.write(&image).is_some()); assert!(sent.write(&image).is_none());
    host.air=host.air.into_iter().map(|((x,y,z,s),m)|((x,y,z+1,s),m)).collect();
    host.deposits=host.deposits.into_iter().map(|((x,y,z),m)|((x,y,z+1),m)).collect();
    let bytes=send(&mut sent,&host); assert_eq!(bytes[0],1);
    assert_eq!(encoded(&read(&bytes,&State::default()).unwrap()),encoded(&host));
}
fn payload(full: u8, s: &State, air: &[[i32;4]], deposits: &[[i32;3]]) -> Vec<u8> {
    let data=encoded(s);let mut b=vec![full];put_u64(&mut b,data.len() as u64);b.extend(data);
    put_u64(&mut b,air.len() as u64);for k in air {for v in k {b.extend(v.to_le_bytes());}}
    put_u64(&mut b,deposits.len() as u64);for k in deposits {for v in k {b.extend(v.to_le_bytes());}}b
}
#[test]
fn ambient_decoder_rejects_malformed_and_over_capacity_deltas() {
    let empty=State::default();let k=[-1,64,0,1];let mut set=State::default();set.air.insert((-1,64,0,1),[1;6]);
    for b in [payload(2,&empty,&[],&[]),payload(1,&empty,&[k],&[]),payload(0,&empty,&[k,k],&[]),
        payload(0,&empty,&[[1_000_001,0,0,1]],&[]),payload(0,&empty,&[[1,0,0,2]],&[]),
        payload(0,&empty,&[[0,0,0,3]],&[]),payload(0,&set,&[k],&[]),
        payload(0,&empty,&[],&[[0,0,0],[0,0,0]])] { assert!(read(&b,&empty).is_err()); }
    let mut bad=payload(1,&set,&[],&[]);bad.push(0);assert!(read(&bad,&empty).is_err());
    for len in 0..bad.len()-1 {assert!(read(&bad[..len],&empty).is_err(),"accepted prefix {len}");}
    let previous=state(1,MAX_CELLS);set.air.clear();set.air.insert((9000,64,0,1),[1;6]);
    assert!(read(&payload(0,&set,&[],&[]),&previous).is_err());
    set.air.clear();set.deposits.insert((9000,64,0),[1;6]);
    assert!(read(&payload(0,&set,&[],&[]),&previous).is_err());
    let mut bad=State::default();bad.target=0;bad.tick=1;
    assert!(read(&payload(1,&bad,&[],&[]),&empty).is_err());
    bad.tick=0;bad.wind[0]=f32::NAN;assert!(read(&payload(1,&bad,&[],&[]),&empty).is_err());
    bad.wind=[0.0;2];bad.imported[0]=(1<<50)+1;assert!(read(&payload(1,&bad,&[],&[]),&empty).is_err());
}
#[test]
fn ambient_sparse_delta_size_is_bounded_at_maximum_residency() {
    let mut host=state(1,MAX_CELLS);let mut sent=Sent::default();let first=send(&mut sent,&host);
    assert_eq!(first.len(),1_016_046);
    let mut peer=read(&first,&State::default()).unwrap();
    for i in 0..32 {
        host.tick+=4;host.target+=4;let k=*host.air.keys().nth(i).unwrap();host.air.get_mut(&k).unwrap()[i%6]+=1;
        let b=send(&mut sent,&host);assert!(b.len()<310,"sparse delta {}",b.len());
        peer=read(&b,&peer).unwrap();assert_eq!(encoded(&host),encoded(&peer));
    }
}
#[test]
fn ambient_differential_churn_keeps_every_species_and_ledger_exact() {
    let mut host=state(4,16);let mut peer=State::default();let mut sent=Sent::default();let mut seed=7u64;
    for tick in 0..240 {
        seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let x=((seed>>32)%64) as i32*4-128;let mass=[seed%100_000;6];
        match seed%4 {0=>{host.air.remove(&(x,256,-128,4));},1=>{host.deposits.remove(&(x,252,-128));},
            2=>{host.air.insert((x,256,-128,4),mass);},_=>{host.deposits.insert((x,252,-128),mass);}}
        host.tick=tick*4;host.target=host.tick+3;host.imported[0]=tick;
        if tick%37==0 { sent=Sent::default(); peer=state(4,3); }
        peer=read(&send(&mut sent,&host),&peer).unwrap();assert_eq!(encoded(&host),encoded(&peer));
    }
}
#[test]
#[ignore = "controlled CPU timing"]
fn ambient_replication_cost() {
    use std::time::Instant;
    for dense in [false,true] { for count in [32,1024,8192] {
        let mut host=state(1,count);let mut sent=Sent::default();let first=send(&mut sent,&host);
        let mut peer=read(&first,&State::default()).unwrap();let mut samples=Vec::new();
        for tick in 0..80 {
            host.tick+=4;host.target+=4;if dense { for m in host.air.values_mut().chain(host.deposits.values_mut()) {m[0]+=1;} }
            else {host.air.first_entry().unwrap().get_mut()[0]+=1;}
            let start=Instant::now();let shared=Arc::new(Image::capture(&host));let sampled=start.elapsed();
            let start=Instant::now();let bytes=sent.write(&shared).unwrap();let capture=start.elapsed();
            let start=Instant::now();peer=read(&bytes,&peer).unwrap();let apply=start.elapsed();
            assert!(sent.write(&shared).is_none());
            if tick>=16 {samples.push((sampled.as_secs_f64()*1e6,capture.as_secs_f64()*1e6,apply.as_secs_f64()*1e6));}
        }
        let p95=|axis:usize| {let mut s:Vec<_>=samples.iter().map(|s|[s.0,s.1,s.2][axis]).collect();s.sort_by(f64::total_cmp);s[s.len()*95/100]};
        println!("[ambient-cost] dense={dense} air={count} deposits={count} full_bytes={} sample_p95_us={:.1} delta_p95_us={:.1} prepare_p95_us={:.1}",first.len(),p95(0),p95(1),p95(2));
        assert_eq!(encoded(&host),encoded(&peer));
    }
    }
}
